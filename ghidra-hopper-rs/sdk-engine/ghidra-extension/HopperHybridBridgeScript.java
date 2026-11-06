// Applies Hopper Disassembler's 5 Core Advantages (ByteType State, Zero-IPC CALL_OBJC, SwitchHint JumpTable, Tags, 4-Mode View) to the active Ghidra Program.
//@category HopperHybrid
//@keybinding ctrl shift H

import java.util.Map;
import ghidra.app.plugin.core.analysis.HopperHybridObjcAnalyzer;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.mem.MemoryBlock;

public class HopperHybridBridgeScript extends GhidraScript {
	@Override
	protected void run() throws Exception {
		if (currentProgram == null) {
			println("No active Program in Ghidra.");
			return;
		}
		HopperHybridObjcAnalyzer analyzer = new HopperHybridObjcAnalyzer();
		Map<Address, String> stubs = analyzer.indexObjcStubsZeroIpc(currentProgram, monitor);
		println("[HopperHybrid] Indexed " + stubs.size() + " __objc_stubs in O(1) without Decompiler IPC.");

		MemoryBlock textBlock = currentProgram.getMemory().getBlock("__TEXT");
		if (textBlock == null) {
			textBlock = currentProgram.getMemory().getBlock("__text");
		}
		if (textBlock != null) {
			byte[] byteStates = HopperHybridObjcAnalyzer.buildHopperByteStateForBlock(
				currentProgram, textBlock, monitor);
			println("[HopperHybrid] Built Hopper ByteType State Map for " + textBlock.getName() +
				" (" + byteStates.length + " bytes).");
		}

		Function func = currentAddress != null
			? currentProgram.getFunctionManager().getFunctionContaining(currentAddress)
			: null;
		if (func == null) {
			FunctionIterator iter = currentProgram.getFunctionManager().getFunctions(true);
			if (iter.hasNext()) {
				func = iter.next();
			}
		}
		if (func != null) {
			println(HopperHybridObjcAnalyzer.renderFourModeFunctionView(
				currentProgram, func, monitor));
		}
	}
}
