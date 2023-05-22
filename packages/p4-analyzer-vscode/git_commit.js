const { execSync } = require('child_process');
const fs = require('fs');

// Get the current Git commit
const commit = execSync('git rev-parse --short HEAD').toString().trim();

// Read the package.json file
const packageJson = JSON.parse(fs.readFileSync('package.json'));

// Update the version field with the commit
packageJson.version = '0.0.1-' + commit.slice(0, 8);

// Write the modified package.json file
fs.writeFileSync('package.json', JSON.stringify(packageJson, null, 2));
