import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import readline from 'readline';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
});

const question = (query) => new Promise((resolve) => rl.question(query, resolve));

function updateJsonFile(filePath, version) {
    const fullPath = path.resolve(rootDir, filePath);
    if (fs.existsSync(fullPath)) {
        const content = fs.readFileSync(fullPath, 'utf8');
        const updated = content.replace(/"version":\s*"[^"]+"/, `"version": "${version}"`);
        fs.writeFileSync(fullPath, updated, 'utf8');
        console.log(`✅ Updated ${filePath}`);
        return true;
    }
    return false;
}

function updateCargoToml(filePath, version) {
    const fullPath = path.resolve(rootDir, filePath);
    if (fs.existsSync(fullPath)) {
        const content = fs.readFileSync(fullPath, 'utf8');
        // Replaces the first occurence of version = "..." which is the package version
        const updated = content.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
        fs.writeFileSync(fullPath, updated, 'utf8');
        console.log(`✅ Updated ${filePath}`);
        return true;
    }
    return false;
}

async function main() {
    const pkgPath = path.resolve(rootDir, 'package.json');
    const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
    const currentVersion = pkg.version;

    console.log(`📦 Current version: \x1b[36m${currentVersion}\x1b[0m`);

    const uncommittedChanges = execSync('git status --porcelain').toString().trim();
    if (uncommittedChanges) {
        console.warn('\x1b[33m⚠️  Warning: You have uncommitted changes in your working directory.\x1b[0m');
        const proceed = await question('Do you want to proceed anyway? (y/N): ');
        if (proceed.toLowerCase() !== 'y') {
            console.log('Aborted.');
            process.exit(0);
        }
    }

    const newVersion = await question(`Enter new version (e.g., 0.8.0): `);

    if (!newVersion || !/^\d+\.\d+\.\d+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/.test(newVersion)) {
        console.error('❌ Invalid version format. Must be x.y.z');
        process.exit(1);
    }

    console.log(`\n🚀 Preparing release for \x1b[32mv${newVersion}\x1b[0m...\n`);

    // Update Files
    const filesToCommit = [];

    const pkgUpdated = updateJsonFile('package.json', newVersion);
    if (pkgUpdated) filesToCommit.push('package.json');

    const tauriUpdated = updateJsonFile('src-tauri/tauri.conf.json', newVersion);
    if (tauriUpdated) filesToCommit.push('src-tauri/tauri.conf.json');

    const cargoUpdated = updateCargoToml('src-tauri/Cargo.toml', newVersion);
    if (cargoUpdated) filesToCommit.push('src-tauri/Cargo.toml');

    console.log('\n...');

    try {
        // Git Commit and Tag
        const filesStr = filesToCommit.join(' ');
        execSync(`git add ${filesStr}`, { stdio: 'inherit', cwd: rootDir });
        execSync(`git commit -m "chore(release): v${newVersion}"`, { stdio: 'inherit', cwd: rootDir });
        execSync(`git tag v${newVersion}`, { stdio: 'inherit', cwd: rootDir });

        console.log(`\n🎉 Successfully tagged \x1b[32mv${newVersion}\x1b[0m`);

        const push = await question('\nDo you want to push the commit and tag to remote now? (Y/n): ');
        if (push.toLowerCase() !== 'n') {
            console.log('Pushing to origin...');
            execSync('git push origin main --tags', { stdio: 'inherit', cwd: rootDir });
            console.log('✅ Pushed successfully.');
        } else {
            console.log('Skipped push. You can push manually using: \x1b[36mgit push origin main --tags\x1b[0m');
        }

    } catch (error) {
        console.error('\x1b[31m❌ Release failed during git operations:\x1b[0m', error.message);
        process.exit(1);
    }

    rl.close();
}

main().catch(console.error);
