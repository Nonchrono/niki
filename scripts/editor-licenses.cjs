const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process');
const dirs=cp.execSync('npm ls --omit=dev --all --parseable',{encoding:'utf8'}).trim().split(/\r?\n/).slice(1);
const sections=[];
for(const dir of dirs){const pkg=JSON.parse(fs.readFileSync(path.join(dir,'package.json'),'utf8'));const license=fs.readdirSync(dir).find(name=>/^licen[cs]e(?:\.md|\.txt)?$/i.test(name));if(!license)throw Error('Missing license for '+pkg.name);sections.push(`${pkg.name}@${pkg.version}\n${fs.readFileSync(path.join(dir,license),'utf8').trim()}\n`);}
fs.writeFileSync('ui/THIRD_PARTY_LICENSES.txt',sections.join('\n---\n\n').trimEnd()+'\n');
