const fs = require('fs')
const path = require('path')

const _package = require('../package.json')
_package.scripts = {}

const outpath = path.join(__dirname, '../dist/package.json')
fs.writeFileSync(outpath, JSON.stringify(_package, null, 2))

const lockpath = path.join(__dirname, '../package-lock.json')
const newlockpath = path.join(__dirname, '../dist/package-lock.json')
fs.copyFileSync(lockpath, newlockpath)

const readmepath = path.join(__dirname, '../README.md')
const newreadmepath = path.join(__dirname, '../dist/README.md')
fs.copyFileSync(readmepath, newreadmepath)

fs.writeFileSync(
  path.join(__dirname, '../dist/.npmignore'),
  `
.DS_Store
*.tmp
*.swp
  `)

try {
  fs.rmSync(path.join(__dirname, '../dist/tests'), { recursive: true})
} catch (_) {}

const distSrc = path.join(__dirname, '../dist/src')
if (fs.existsSync(distSrc)) {
  const files = fs.readdirSync(distSrc)
  for (const file of files) {
    fs.renameSync(path.join(distSrc, file), path.join(__dirname, '../dist', file))
  }
  fs.rmSync(distSrc, { recursive: true})
}
