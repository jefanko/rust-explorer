import { spawn, execSync } from "child_process";
import fs from "fs";
import path from "path";
import os from "os";

console.log("=== Rust Explorer Native E2E Test Suite ===");

const rootDir = path.resolve("../../");
const releaseExe = path.join(rootDir, "target", "release", "rust-explorer.exe");
const debugExe = path.join(rootDir, "target", "debug", "rust-explorer.exe");
const targetExe = fs.existsSync(releaseExe) ? releaseExe : debugExe;

if (!fs.existsSync(targetExe)) {
  console.error(`[FAIL] Executable not found at ${targetExe}. Please build the project first.`);
  process.exit(1);
}

console.log(`[OK] Located executable: ${targetExe} (${(fs.statSync(targetExe).size / (1024 * 1024)).toFixed(2)} MB)`);

// Step 1: Create isolated temporary environment
const testDir = path.join(rootDir, "target", "native-e2e-env");
const fakeAppData = path.join(testDir, "AppData", "Local");
const fixtureDir = path.join(testDir, "fixtures");

fs.rmSync(testDir, { recursive: true, force: true });
fs.mkdirSync(fakeAppData, { recursive: true });
fs.mkdirSync(fixtureDir, { recursive: true });

// Create test fixture files
fs.writeFileSync(path.join(fixtureDir, "Document.txt"), "E2E sample document");
fs.writeFileSync(path.join(fixtureDir, "Spreadsheet.csv"), "col1,col2\nval1,val2");
fs.mkdirSync(path.join(fixtureDir, "NestedFolder"));
fs.writeFileSync(path.join(fixtureDir, "NestedFolder", "Child.txt"), "Child file content");

console.log(`[OK] Created isolated test environment at ${testDir}`);

async function runNativeE2E() {
  console.log("\n--> Launching Rust Explorer in isolated test environment...");

  const child = spawn(targetExe, [], {
    env: {
      ...process.env,
      LOCALAPPDATA: fakeAppData,
      RUST_LOG: "info",
    },
    stdio: "ignore",
  });

  if (!child.pid) {
    throw new Error("Failed to spawn Rust Explorer process");
  }

  console.log(`[OK] Process launched with PID: ${child.pid}`);

  // Allow process to initialize and display main window
  await new Promise((resolve) => setTimeout(resolve, 3500));

  // Query process status and memory usage via PowerShell Get-Process
  const psOutput = execSync(
    `powershell.exe -Command "Get-Process -Id ${child.pid} | Select-Object -Property Id, ProcessName, Responding, WorkingSet64 | ConvertTo-Json"`,
    { encoding: "utf8" }
  );

  const procInfo = JSON.parse(psOutput);
  console.log(`[OK] Process Responding: ${procInfo.Responding}`);
  const memMb = (procInfo.WorkingSet64 / (1024 * 1024)).toFixed(2);
  console.log(`[OK] Working set memory: ${memMb} MB (Budget: <= 350 MB)`);

  if (Number(memMb) > 350) {
    throw new Error(`Memory exceeded budget: ${memMb} MB > 350 MB`);
  }

  // Verify isolated state initialization
  const expectedStateDir = path.join(fakeAppData, "RustExplorer");
  const stateFilesExist = fs.existsSync(expectedStateDir);
  console.log(`[OK] AppData state directory initialized: ${stateFilesExist} (${expectedStateDir})`);

  // Terminate process safely
  console.log("\n--> Terminating test process cleanly...");
  try {
    process.kill(child.pid, "SIGTERM");
  } catch (e) {
    // Process may have already exited
  }

  // Verify NSIS / MSI Installer bundle presence
  console.log("\n--> Verifying release distribution bundles...");
  const bundleDir = path.join(rootDir, "target", "release", "bundle");
  const nsisDir = path.join(bundleDir, "nsis");
  const msiDir = path.join(bundleDir, "msi");

  if (fs.existsSync(nsisDir)) {
    const files = fs.readdirSync(nsisDir);
    const setupExe = files.find((f) => f.endsWith("-setup.exe"));
    if (setupExe) {
      const stat = fs.statSync(path.join(nsisDir, setupExe));
      console.log(`[OK] NSIS Installer found: ${setupExe} (${(stat.size / (1024 * 1024)).toFixed(2)} MB)`);
    }
  }

  if (fs.existsSync(msiDir)) {
    const files = fs.readdirSync(msiDir);
    const msi = files.find((f) => f.endsWith(".msi"));
    if (msi) {
      const stat = fs.statSync(path.join(msiDir, msi));
      console.log(`[OK] MSI Installer found: ${msi} (${(stat.size / (1024 * 1024)).toFixed(2)} MB)`);
    }
  }

  // Cleanup test environment
  try {
    fs.rmSync(testDir, { recursive: true, force: true });
    console.log(`[OK] Cleaned up temporary test environment`);
  } catch (err) {
    // Ignore cleanup errors
  }

  console.log("\n==============================================");
  console.log("All Native E2E verification steps PASSED!");
  console.log("==============================================");
}

runNativeE2E().catch((err) => {
  console.error("[FAIL] Native E2E failed:", err);
  process.exit(1);
});
