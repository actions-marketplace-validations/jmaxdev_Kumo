const fs = require('fs');
const path = require('path');
const readline = require('readline');

const mainCratePath = path.join(__dirname, '..', 'crates', 'cli', 'Cargo.toml');
const manifest = fs.readFileSync(mainCratePath, 'utf8');
const versionMatch = manifest.match(/^version = "(.*)"/m);

if (!versionMatch) {
    console.error('Could not find version in crates/cli/Cargo.toml');
    process.exit(1);
}

let currentVersion = versionMatch[1];
let [major, minor, patch] = currentVersion.split('.').map(Number);

async function run() {
    let mode = process.argv[2];

    if (!mode) {
        const rl = readline.createInterface({
            input: process.stdin,
            output: process.stdout
        });

        console.log(`📦 Kumo Version Bumper (Current: ${currentVersion})`);
        console.log('Select bump type:');
        console.log(`1) Patch (${major}.${minor}.${patch + 1})`);
        console.log(`2) Minor (${major}.${minor + 1}.0)`);
        console.log(`3) Major (${major + 1}.0.0)`);
        console.log('4) Custom Version');

        const choice = await new Promise(resolve => rl.question('Choice: ', resolve));
        
        if (choice === '1') mode = 'patch';
        else if (choice === '2') mode = 'minor';
        else if (choice === '3') mode = 'major';
        else if (choice === '4') {
            mode = await new Promise(resolve => rl.question('Enter version: ', resolve));
        } else {
            console.log('Invalid choice.');
            process.exit(1);
        }
        rl.close();
    }

    let newVersion;
    if (mode === 'major') {
        newVersion = `${major + 1}.0.0`;
    } else if (mode === 'minor') {
        newVersion = `${major}.${minor + 1}.0`;
    } else if (mode === 'patch') {
        newVersion = `${major}.${minor}.${patch + 1}`;
    } else {
        newVersion = mode.startsWith('v') ? mode.slice(1) : mode;
    }

    console.log(`🚀 Bumping version: ${currentVersion} -> ${newVersion}`);

    const cratesDir = path.join(__dirname, '..', 'crates');
    const crates = fs.readdirSync(cratesDir);

    crates.forEach(crate => {
        const cargoPath = path.join(cratesDir, crate, 'Cargo.toml');
        if (fs.existsSync(cargoPath)) {
            console.log(`  Updating ${crate}...`);
            let content = fs.readFileSync(cargoPath, 'utf8');
            content = content.replace(/^version = ".*"/m, `version = "${newVersion}"`);
            fs.writeFileSync(cargoPath, content);
        }
    });

    console.log('✅ Version update complete!');

    try {
        const { execSync } = require('child_process');
        console.log('📦 Committing and tagging...');
        
        execSync('git add .');
        execSync(`git commit -m "release: v${newVersion}"`);
        execSync(`git tag v${newVersion}`);
        
        console.log(`📤 Pushing changes and tag v${newVersion} to origin...`);
        execSync('git push origin master');
        execSync(`git push origin v${newVersion}`);
        
        console.log(`✨ Successfully released v${newVersion}! GitHub Actions will now build the artifacts.`);
    } catch (error) {
        console.warn('⚠️ Git operations failed. Please make sure you have git installed and are in a git repository.');
        console.log(`💡 Manual steps needed:\n   git commit -am "release: v${newVersion}"\n   git tag v${newVersion}\n   git push origin v${newVersion}`);
    }
}

run();
