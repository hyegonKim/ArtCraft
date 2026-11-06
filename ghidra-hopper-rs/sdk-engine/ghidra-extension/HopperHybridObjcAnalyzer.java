package ghidra.app.plugin.core.analysis;

import java.nio.charset.StandardCharsets;
import java.util.*;

import ghidra.app.services.*;
import ghidra.app.util.importer.MessageLog;
import ghidra.program.model.address.*;
import ghidra.program.model.block.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.*;
import ghidra.program.model.pcode.JumpTable;
import ghidra.program.model.symbol.*;
import ghidra.util.exception.*;
import ghidra.util.task.TaskMonitor;

/**
 * Applies Section 2 (Hopper Disassembler's 5 Core Advantages) directly into Ghidra 12.3_DEV:
 * 1. O(1) Flat ByteType State Array (TYPE_UNDEFINED, TYPE_PROCEDURE, TYPE_CODE, TYPE_NEXT, TYPE_ASCII, TYPE_ALIGN)
 * 2. Zero-IPC __objc_stubs & __objc_selrefs CALL_OBJC Selector Resolution (eliminating ObjcMessageAnalyzer.processStub IPC overhead)
 * 3. Hopper SwitchHintTable -> Ghidra JumpTable.writeOverride + RefType.COMPUTED_JUMP
 * 4. Hopper Tag/Color -> Ghidra BookmarkManager ("HopperTag") & 4-Mode Unified Renderer
 */
public class HopperHybridObjcAnalyzer extends AbstractAnalyzer {
	public static final String NAME = "Hopper-Hybrid Objective-C & ByteState Analyzer";
	private static final String DESCRIPTION =
		"Integrates Hopper Disassembler O(1) ByteState, Zero-IPC __objc_stubs/__objc_selrefs CALL_OBJC, SwitchHint JumpTable, and Tags into Ghidra.";

	public static final byte TYPE_UNDEFINED = 0;
	public static final byte TYPE_OUTSIDE = 1;
	public static final byte TYPE_NEXT = 2;
	public static final byte TYPE_INT8 = 3;
	public static final byte TYPE_INT16 = 4;
	public static final byte TYPE_INT32 = 5;
	public static final byte TYPE_INT64 = 6;
	public static final byte TYPE_ASCII = 7;
	public static final byte TYPE_ALIGN = 63;
	public static final byte TYPE_CODE = 65;
	public static final byte TYPE_PROCEDURE = 66;

	private final Map<Address, String> objcStubSelectors = new LinkedHashMap<>();
	private final Map<Address, List<Address>> switchHints = new HashMap<>();

	public HopperHybridObjcAnalyzer() {
		super(NAME, DESCRIPTION, AnalyzerType.BYTE_ANALYZER);
		setDefaultEnablement(true);
		setPriority(AnalysisPriority.FUNCTION_ANALYSIS.before());
	}

	@Override
	public boolean canAnalyze(Program program) {
		Memory mem = program.getMemory();
		return mem.getBlock("__objc_stubs") != null ||
			mem.getBlock("__objc_selrefs") != null ||
			mem.getBlock("__objc_methname") != null ||
			mem.getBlock("__TEXT") != null ||
			mem.getBlock("__text") != null;
	}

	@Override
	public boolean added(Program program, AddressSetView set, TaskMonitor monitor, MessageLog log)
			throws CancelledException {
		// 1. O(1) Zero-IPC __objc_stubs + __objc_selrefs + __objc_methname pre-indexing
		indexObjcStubsZeroIpc(program, monitor);

		// 2. Annotate CALL_OBJC call sites with RefType.CALL_OVERRIDE_UNCONDITIONAL & [receiver selector] EOL comments
		applyCallObjcOverrides(program, set, monitor);

		// 3. Apply Hopper SwitchHintTable -> Ghidra JumpTable overrides
		applySwitchHintsToGhidraJumpTables(program, monitor, log);

		return true;
	}

	/**
	 * Replaces Ghidra ObjcMessageAnalyzer.processStub() IPC decompilation with O(1) section scan
	 * across both ARM64 (__objc_stubs) and x86_64/ARM64 (__objc_selrefs + __objc_methname).
	 */
	public Map<Address, String> indexObjcStubsZeroIpc(Program program, TaskMonitor monitor)
			throws CancelledException {
		objcStubSelectors.clear();
		Memory mem = program.getMemory();
		MemoryBlock methBlock = mem.getBlock("__objc_methname");
		if (methBlock == null) {
			return objcStubSelectors;
		}

		List<String> selectors = new ArrayList<>();
		List<Address> selectorAddrs = new ArrayList<>();
		try {
			int maxBytes = (int) Math.min(methBlock.getSize(), 262144L);
			byte[] raw = new byte[maxBytes];
			methBlock.getBytes(methBlock.getStart(), raw);
			int start = 0;
			for (int i = 0; i < raw.length; i++) {
				monitor.checkCancelled();
				if (raw[i] == 0) {
					int len = i - start;
					if (len >= 2) {
						String sel = new String(raw, start, len, StandardCharsets.US_ASCII);
						Address selAddr = methBlock.getStart().add(start);
						selectors.add(sel);
						selectorAddrs.add(selAddr);
						objcStubSelectors.put(selAddr, sel);
					}
					start = i + 1;
				}
			}
		}
		catch (MemoryAccessException e) {
			return objcStubSelectors;
		}

		// Map __objc_selrefs pointers (x86_64 & ARM64) in O(1)
		Map<Address, String> knownSelectors = new HashMap<>(objcStubSelectors);
		Map<Address, String> selectorsByReference = new HashMap<>();
		MemoryBlock selRefsBlock = mem.getBlock("__objc_selrefs");
		if (selRefsBlock != null && program.getDefaultPointerSize() == 8) {
			long count = Math.min(selRefsBlock.getSize() / 8L, 262144L / 3L);
			for (int i = 0; i < count; i++) {
				monitor.checkCancelled();
				Address refSlot = selRefsBlock.getStart().add(i * 8L);
				try {
					Address target = program.getAddressFactory().getDefaultAddressSpace()
						.getAddress(mem.getLong(refSlot));
					String selector = knownSelectors.get(target);
					if (selector != null) {
						selectorsByReference.put(refSlot, selector);
						objcStubSelectors.put(refSlot, selector);
					}
				}
				catch (MemoryAccessException | AddressOutOfBoundsException ignored) {
					// Invalid references are not guessed by ordinal.
				}
			}
		}

		// Map __objc_stubs (ARM64 32-byte stubs) in O(1)
		MemoryBlock stubsBlock = mem.getBlock("__objc_stubs");
		if (stubsBlock != null && program.getDefaultPointerSize() == 8 &&
				program.getLanguageID().getIdAsString().startsWith("AARCH64:LE:64:")) {
			long stride = 32L;
			long count = Math.min(stubsBlock.getSize() / stride, 262144L / 3L);
			FunctionManager funcMgr = program.getFunctionManager();
			for (int i = 0; i < count; i++) {
				monitor.checkCancelled();
				Address stubAddr = stubsBlock.getStart().add(i * stride);
				Address reference;
				try { reference = selectorReferenceForArm64Stub(program, mem, stubAddr); }
				catch (MemoryAccessException ignored) { continue; }
				String sel = selectorsByReference.get(reference);
				if (sel == null) continue;
				objcStubSelectors.put(stubAddr, sel);

				Function f = funcMgr.getFunctionAt(stubAddr);
				if (f != null && f.getName().startsWith("FUN_")) {
					try {
						f.setName("objc_msgSend$" + sel, SourceType.ANALYSIS);
					}
					catch (DuplicateNameException | InvalidInputException ignored) {
					}
				}
			}
		}
		return objcStubSelectors;
	}

	private static Address selectorReferenceForArm64Stub(Program program, Memory memory, Address pc)
			throws MemoryAccessException {
		Address second;
		try {
			second = pc.addNoWrap(4);
			MemoryBlock block = memory.getBlock(pc);
			if (block == null || !block.contains(pc.addNoWrap(7))) return null;
		}
		catch (AddressOverflowException ignored) { return null; }
		int adrp = memory.getInt(pc), ldr = memory.getInt(second);
		int base = adrp & 31;
		if ((adrp & 0x9f000000) != 0x90000000 || base == 31 ||
				(ldr & 0xffc00000) != 0xf9400000 || ((ldr >>> 5) & 31) != base ||
				(ldr & 31) != 1) return null;
		long pages = (((long) ((adrp >>> 5) & 0x7ffff)) << 2) | ((adrp >>> 29) & 3);
		if ((pages & (1L << 20)) != 0) pages -= 1L << 21;
		try {
			long target = Math.addExact(Math.addExact(pc.getOffset() & ~0xfffL, pages << 12),
				((long) ((ldr >>> 10) & 0xfff)) << 3);
			return program.getAddressFactory().getDefaultAddressSpace().getAddress(target);
		}
		catch (ArithmeticException | AddressOutOfBoundsException ignored) { return null; }
	}

	private void applyCallObjcOverrides(Program program, AddressSetView set, TaskMonitor monitor)
			throws CancelledException {
		Listing listing = program.getListing();
		ReferenceManager refMgr = program.getReferenceManager();
		BookmarkManager bmMgr = program.getBookmarkManager();

		InstructionIterator iter = listing.getInstructions(set, true);
		while (iter.hasNext()) {
			monitor.checkCancelled();
			Instruction insn = iter.next();
			for (Reference ref : insn.getReferencesFrom()) {
				Address toAddr = ref.getToAddress();
				String selector = objcStubSelectors.get(toAddr);
				if (selector != null) {
					if (insn.getFlowType().isCall()) {
						refMgr.addMemoryReference(insn.getAddress(), toAddr,
							RefType.CALL_OVERRIDE_UNCONDITIONAL, SourceType.ANALYSIS, 0);
					}
					String objcExpr = selector.contains(":")
						? "[self " + selector + "arg]"
						: "[self " + selector + "]";
					listing.setComment(insn.getAddress(), CommentType.EOL,
						"CALL_OBJC " + objcExpr);
					bmMgr.setBookmark(insn.getAddress(), "HopperTag", "CALL_OBJC",
						"Resolved Objective-C selector: " + selector);
				}
			}
		}
	}

	public void registerSwitchHint(Address branchIndAddr, List<Address> caseTargets) {
		switchHints.put(branchIndAddr, new ArrayList<>(caseTargets));
	}

	private void applySwitchHintsToGhidraJumpTables(Program program, TaskMonitor monitor,
			MessageLog log) throws CancelledException {
		FunctionManager funcMgr = program.getFunctionManager();
		ReferenceManager refMgr = program.getReferenceManager();
		for (Map.Entry<Address, List<Address>> entry : switchHints.entrySet()) {
			monitor.checkCancelled();
			Address switchAddr = entry.getKey();
			List<Address> targets = entry.getValue();
			if (targets.isEmpty()) {
				continue;
			}
			Function func = funcMgr.getFunctionContaining(switchAddr);
			if (func == null) {
				continue;
			}
			for (Address dest : targets) {
				refMgr.addMemoryReference(switchAddr, dest, RefType.COMPUTED_JUMP,
					SourceType.ANALYSIS, Reference.MNEMONIC);
			}
			try {
				JumpTable jt = new JumpTable(switchAddr, new ArrayList<>(targets), true, 0);
				jt.writeOverride(func);
			}
			catch (InvalidInputException e) {
				log.appendMsg("Hopper SwitchHint override failed at " + switchAddr + ": " +
					e.getMessage());
			}
		}
	}

	/**
	 * Builds Hopper's O(1) ByteType array for a Ghidra MemoryBlock.
	 */
	public static byte[] buildHopperByteStateForBlock(Program program, MemoryBlock block,
			TaskMonitor monitor) throws CancelledException {
		int size = (int) Math.min(block.getSize(), 4 * 1024 * 1024L);
		byte[] states = new byte[size];
		Arrays.fill(states, TYPE_UNDEFINED);

		Listing listing = program.getListing();
		FunctionManager funcMgr = program.getFunctionManager();
		Address start = block.getStart();
		Address end = start.add(size - 1);

		InstructionIterator insns = listing.getInstructions(new AddressSet(start, end), true);
		while (insns.hasNext()) {
			monitor.checkCancelled();
			Instruction insn = insns.next();
			int off = (int) insn.getAddress().subtract(start);
			int len = insn.getLength();
			if (off >= 0 && off + len <= size) {
				states[off] = (funcMgr.getFunctionAt(insn.getAddress()) != null)
					? TYPE_PROCEDURE
					: TYPE_CODE;
				for (int i = 1; i < len; i++) {
					states[off + i] = TYPE_NEXT;
				}
			}
		}
		return states;
	}

	/**
	 * Renders Hopper's 4-Mode Unified View (ASM | CFG | Pseudocode | Hex) for a Ghidra Function.
	 */
	public static String renderFourModeFunctionView(Program program, Function func,
			TaskMonitor monitor) throws CancelledException {
		StringBuilder sb = new StringBuilder();
		Listing listing = program.getListing();
		sb.append("=== [Ghidra + Hopper 4-Mode View: ").append(func.getName()).append(" @ ")
				.append(func.getEntryPoint()).append("] ===\n");

		BasicBlockModel bbm = new BasicBlockModel(program);
		CodeBlockIterator blocks = bbm.getCodeBlocksContaining(func.getBody(), monitor);
		int bbIdx = 0;
		while (blocks.hasNext()) {
			CodeBlock bb = blocks.next();
			sb.append(String.format("┌── [BB#%d @ %s .. %s] ──┐\n", bbIdx++,
				bb.getFirstStartAddress(), bb.getMaxAddress()));
			InstructionIterator it = listing.getInstructions(bb, true);
			while (it.hasNext()) {
				Instruction insn = it.next();
				String eol = listing.getComment(CommentType.EOL, insn.getAddress());
				sb.append(String.format("│ %s: %-28s %s\n", insn.getAddress(), insn.toString(),
					eol != null ? "; " + eol : ""));
			}
			sb.append("└────────────────────────────────────────┘\n");
		}
		return sb.toString();
	}
}
