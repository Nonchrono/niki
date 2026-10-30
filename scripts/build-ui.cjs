const fs = require('node:fs/promises');
const path = require('node:path');
const esbuild = require('esbuild');
const postcss = require('postcss');
const tailwind = require('@tailwindcss/postcss');

async function main() {
  await esbuild.build({
    entryPoints: ['ui-src/main.tsx'], bundle: true, minify: true,
    format: 'iife', target: 'es2020', supported: { 'template-literal': false },
    outfile: 'ui/app.js', legalComments: 'none',
  });
  const input = path.resolve('ui-src/styles/tailwind.css');
  const output = path.resolve('ui/style.css');
  const source = await fs.readFile(input, 'utf8');
  const result = await postcss([tailwind()]).process(source, { from: input, to: output });
  await fs.writeFile(output, result.css);
  for (const warning of result.warnings()) console.warn(warning.toString());
}

main().catch(error => { console.error(error); process.exitCode = 1; });
