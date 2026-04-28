import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const contractPath = path.resolve(__dirname, '../../contracts/src/contract.rs');
const errorsPath = path.resolve(__dirname, '../../contracts/src/errors.rs');
const bindingsPath = path.resolve(__dirname, './index.ts');

if (!fs.existsSync(contractPath) || !fs.existsSync(bindingsPath)) {
    console.error(`Could not find required files.\nContract: ${contractPath}\nBindings: ${bindingsPath}`);
    process.exit(1);
}

if (!fs.existsSync(errorsPath)) {
    console.error(`Could not find errors file.\nErrors: ${errorsPath}`);
    process.exit(1);
}

const contractCode = fs.readFileSync(contractPath, 'utf8');
const errorsCode = fs.readFileSync(errorsPath, 'utf8');
const bindingsCode = fs.readFileSync(bindingsPath, 'utf8');

// Parse contract exports inside `impl VirtualTokenContract`
const contractFns = [];
const contractSegments = contractCode.split('impl VirtualTokenContract');
if (contractSegments.length > 1) {
    // Use everything after `impl VirtualTokenContract` up to the closing brace? Just parse the rest of the file
    const implBlock = contractSegments[1];
    const lines = implBlock.split('\n');
    for (const line of lines) {
        // looking for things like: `pub fn create_round(`
        const match = line.match(/^\s*pub\s+fn\s+([a-zA-Z0-9_]+)\s*\(/);
        const isPubCrate = line.match(/^\s*pub\(crate\)\s+fn/);
        if (match && !isPubCrate) {
            contractFns.push(match[1]);
        }
    }
}

// Parse bindings for exported methods listed in `fromJSON` block
const bindingsFns = [];
const bindingsSegments = bindingsCode.split('public readonly fromJSON = {');
if (bindingsSegments.length > 1) {
    const fromJsonBlock = bindingsSegments[1].split('}')[0];
    const lines = fromJsonBlock.split('\n');
    for (const line of lines) {
        const match = line.match(/(?:^\s*|\s+)([a-zA-Z0-9_]+)\s*:\s*this\.txFromJSON/);
        if (match) {
            bindingsFns.push(match[1]);
        }
    }
}

if (contractFns.length === 0) {
    console.error("Failed to parse contract functions from:", contractPath);
    process.exit(1);
}

if (bindingsFns.length === 0) {
    console.error("Failed to parse binding functions from:", bindingsPath);
    process.exit(1);
}

// ─── Error code parity ────────────────────────────────────────────────────────

// Parse error variants from errors.rs: `VariantName = N,`
const contractErrors = new Map();
for (const line of errorsCode.split('\n')) {
    const match = line.match(/^\s+([A-Za-z][A-Za-z0-9]+)\s*=\s*(\d+)\s*,?\s*$/);
    if (match) {
        contractErrors.set(parseInt(match[2], 10), match[1]);
    }
}

// Parse error entries from index.ts: `N: {message:"VariantName"},`
const bindingsErrors = new Map();
for (const line of bindingsCode.split('\n')) {
    const match = line.match(/^\s+(\d+)\s*:\s*\{message\s*:\s*"([^"]+)"\}/);
    if (match) {
        bindingsErrors.set(parseInt(match[1], 10), match[2]);
    }
}

if (contractErrors.size === 0) {
    console.error("Failed to parse error codes from:", errorsPath);
    process.exit(1);
}

if (bindingsErrors.size === 0) {
    console.error("Failed to parse error codes from:", bindingsPath);
    process.exit(1);
}

const errorsMissingInBindings = [];
const errorsMismatch = [];
for (const [code, name] of contractErrors) {
    if (!bindingsErrors.has(code)) {
        errorsMissingInBindings.push({ code, name });
    } else if (bindingsErrors.get(code) !== name) {
        errorsMismatch.push({ code, contractName: name, bindingsName: bindingsErrors.get(code) });
    }
}
const errorsOnlyInBindings = [];
for (const [code, name] of bindingsErrors) {
    if (!contractErrors.has(code)) {
        errorsOnlyInBindings.push({ code, name });
    }
}

// ─── Report ───────────────────────────────────────────────────────────────────

const fnMissingInBindings = contractFns.filter(fn => !bindingsFns.includes(fn));
const fnMissingInContract = bindingsFns.filter(fn => !contractFns.includes(fn));

let failed = false;

if (fnMissingInBindings.length > 0 || fnMissingInContract.length > 0) {
    failed = true;
    console.error("❌ ABI parity check failed: Drift detected");

    if (fnMissingInBindings.length > 0) {
        console.error("- The following methods are present in the contract but missing from the bindings map:");
        fnMissingInBindings.forEach(fn => console.error(`  - ${fn}`));
    }

    if (fnMissingInContract.length > 0) {
        console.error("- The following methods are in the bindings map but missing from the contract:");
        fnMissingInContract.forEach(fn => console.error(`  - ${fn}`));
    }
}

if (errorsMissingInBindings.length > 0 || errorsMismatch.length > 0 || errorsOnlyInBindings.length > 0) {
    failed = true;
    console.error("❌ Error code parity check failed: Drift detected");

    if (errorsMissingInBindings.length > 0) {
        console.error("- Contract errors missing from bindings:");
        errorsMissingInBindings.forEach(({ code, name }) => console.error(`  - ${code}: ${name}`));
    }

    if (errorsMismatch.length > 0) {
        console.error("- Error name mismatches (contract vs bindings):");
        errorsMismatch.forEach(({ code, contractName, bindingsName }) =>
            console.error(`  - ${code}: contract="${contractName}" bindings="${bindingsName}"`)
        );
    }

    if (errorsOnlyInBindings.length > 0) {
        console.error("- Bindings errors not present in contract:");
        errorsOnlyInBindings.forEach(({ code, name }) => console.error(`  - ${code}: ${name}`));
    }
}

if (failed) {
    process.exit(1);
} else {
    console.log("✅ ABI parity check passed: All contract methods and error codes are synced with TS bindings.");
    process.exit(0);
}
