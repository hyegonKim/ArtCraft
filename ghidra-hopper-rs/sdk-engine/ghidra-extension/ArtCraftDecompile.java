// Owned, one-way headless exporter. No target executable is launched.
//@category ArtCraft
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import com.google.gson.*;
import ghidra.framework.Application;
import ghidra.app.util.headless.HeadlessScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class ArtCraftDecompile extends HeadlessScript {
    private static String sha(Path path) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (var stream = Files.newInputStream(path)) {
            byte[] chunk = new byte[65536];
            for (int n; (n = stream.read(chunk)) >= 0;) digest.update(chunk, 0, n);
        }
        return java.util.HexFormat.of().formatHex(digest.digest());
    }
    @Override public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 2) throw new IllegalArgumentException("request and result paths required");
        JsonObject request = JsonParser.parseString(Files.readString(Path.of(args[0]))).getAsJsonObject();
        JsonObject out = new JsonObject();
        out.addProperty("schema", 1);
        out.addProperty("request_id", request.get("request_id").getAsString());
        out.addProperty("status", "error");
        try {
            if (currentProgram == null) throw new IllegalStateException("no imported program");
            if (analysisTimeoutOccurred()) throw new IllegalStateException("analysis exceeded its time limit");
            String entryText = request.get("entry").getAsString();
            Address entry = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(
                Long.parseUnsignedLong(entryText.substring(2), 16));
            Function function = currentProgram.getFunctionManager().getFunctionAt(entry);
            if (function == null) throw new IllegalStateException("no function starts at the requested entry");
            Path input = Path.of(request.get("input_path").getAsString());
            String inputSha = sha(input);
            if (!inputSha.equalsIgnoreCase(currentProgram.getExecutableSHA256()))
                throw new IllegalStateException("imported program/input SHA-256 mismatch");
            DecompInterface decompiler = new DecompInterface();
            try {
                if (!decompiler.openProgram(currentProgram))
                    throw new IllegalStateException("native decompiler could not open the imported program");
                DecompileResults result = decompiler.decompileFunction(function, 30, monitor);
                if (!result.decompileCompleted() || result.getHighFunction() == null || result.getDecompiledFunction() == null)
                    throw new IllegalStateException("native function decompilation did not complete: " + result.getErrorMessage());
                String c = result.getDecompiledFunction().getC();
                if (c.isBlank() || c.getBytes(StandardCharsets.UTF_8).length > 1048576)
                    throw new IllegalStateException("native C is empty or exceeds the 1 MiB output limit");
                out.addProperty("c", c);
            } finally { decompiler.dispose(); }
            Path nativeFile = Application.getOSFile("decompile").toPath().toRealPath();
            Path utilityJar = Path.of(ghidra.Ghidra.class.getProtectionDomain()
                .getCodeSource().getLocation().toURI()).toRealPath();
            // .../Ghidra/Framework/Utility/lib/Utility.jar -> installation root.
            Path sdk = utilityJar;
            for (int i = 0; i < 5; i++) sdk = sdk.getParent();
            out.addProperty("sdk_home", sdk.toString());
            out.addProperty("sdk_version", Application.getApplicationVersion());
            out.addProperty("java_home", Path.of(System.getProperty("java.home")).toRealPath().toString());
            out.addProperty("java_version", System.getProperty("java.version"));
            out.addProperty("native_path", nativeFile.toString());
            out.addProperty("native_sha256", sha(nativeFile));
            out.addProperty("exporter_sha256", sha(Path.of(request.get("exporter_path").getAsString())));
            out.addProperty("input_sha256", inputSha);
            out.addProperty("input_bytes", Files.size(input));
            out.addProperty("architecture", currentProgram.getLanguage().getProcessor().toString());
            out.addProperty("entry", "0x" + Long.toUnsignedString(function.getEntryPoint().getOffset(), 16));
            out.addProperty("function_name", function.getName());
            out.addProperty("high_function", true);
            out.addProperty("decompile_completed", true);
            out.addProperty("analysis_timed_out", false);
            out.addProperty("status", "complete");
        } catch (Exception error) {
            String message = error.getClass().getSimpleName() + ": " + error.getMessage();
            out.addProperty("error", message.substring(0, Math.min(message.length(), 4096)));
        }
        byte[] bytes = new Gson().toJson(out).getBytes(StandardCharsets.UTF_8);
        if (bytes.length > 2097152) throw new IllegalStateException("result exceeds the 2 MiB JSON limit");
        Files.write(Path.of(args[1]), bytes, StandardOpenOption.CREATE_NEW);
    }
}
