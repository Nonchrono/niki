const {chromium}=require('playwright-core');
const assert=require('node:assert/strict');
const {pathToFileURL}=require('node:url');const path=require('node:path');
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_PATH||'C:/Program Files/Google/Chrome/Application/chrome.exe',args:['--allow-file-access-from-files']});
 const page=await browser.newPage();
 await page.addInitScript(()=>{
  window.sample={locked:false,updater_config:{},journals:[{id:'j',name:'일기',color:'#576b61'}],entries:[{id:'e',journal_id:'j',title:'기록',body:'기존',date:'2026-10-29',updated_at:'v1',tags:[],favorite:false,attachments:[],location:'',weather:''}],templates:[],sync_conflicts:[],pending_resolution_count:0,synced_entry_ids:[],sync_status:{connected:false}};
  window.calls=[];window.hold=false;
  window.__TAURI__={core:{invoke:async(name,args)=>{
   calls.push(name);if(name==='bootstrap')return structuredClone(sample);
   if(name==='save_entry_checked'){const e=sample.entries[0];if(JSON.stringify(args.input.expectedEntry)!==JSON.stringify(e))throw Error('다른 기기에서 이 기록이 변경되었습니다');Object.assign(e,{body:args.input.body,title:args.input.title,date:args.input.date,journal_id:args.input.journalId,favorite:args.input.favorite,tags:args.input.tags,location:args.input.location,weather:args.input.weather,updated_at:'v'+calls.length});return {id:e.id,entry:structuredClone(e)};}
   if(name==='save_entry'){const e={...sample.entries[0],id:'copy',body:args.input.body,title:args.input.title,updated_at:'copy-version',attachments:[]};sample.entries.push(e);return e.id;}
   if(name==='drive_sync_now'){if(hold)await new Promise(resolve=>window.releaseSync=resolve);return {warnings:[]};}throw Error(name);
  }}};
 });
 await page.goto(pathToFileURL(path.resolve('ui/index.html')).href);await page.locator('[data-action="entry"]').click();
 await page.evaluate(async()=>{sample.sync_status={connected:true};await reload();});
 await page.locator('.ProseMirror').click();await page.keyboard.type(' 변경');await page.waitForTimeout(800);
 assert(await page.evaluate(()=>calls.includes('save_entry_checked')));assert(await page.evaluate(()=>syncTimer!==null),'save must schedule automatic synchronization');
 await page.evaluate(()=>scheduleAutoSync(10));await page.waitForFunction(()=>calls.includes('drive_sync_now'));
 assert((await page.locator('.ProseMirror').innerText()).includes('변경'));
 await page.evaluate(()=>{window.hold=true;autoSyncDrive();});await page.waitForFunction(()=>typeof releaseSync==='function');
 await page.evaluate(()=>{sample.entries[0].body='원격';sample.entries[0].updated_at='remote';});await page.keyboard.type(' 미저장');
 await page.evaluate(()=>releaseSync());await page.waitForTimeout(1000);
 assert((await page.locator('.ProseMirror').innerText()).includes('미저장'));assert.equal(await page.evaluate(()=>sample.entries[0].body),'원격');assert((await page.locator('#save-status').innerText()).includes('기록이 변경'));assert.equal(await page.locator('[data-action="save-draft-copy"]').count(),1);
 page.once('dialog',dialog=>dialog.accept());await page.reload();await page.locator('[data-action="entry"]').click();await page.waitForTimeout(900);
 assert((await page.locator('.ProseMirror').innerText()).includes('미저장'),'unsaved draft survives reload');assert.equal(await page.locator('[data-action="save-draft-copy"]').count(),1);
 await page.locator('[data-action="save-draft-copy"]').click();assert((await page.evaluate(()=>sample.entries.find(e=>e.id==='copy').body)).includes('미저장'));assert.equal(await page.evaluate(()=>sample.entries[0].body),'기존');
 await browser.close();console.log('Automatic sync scheduling and draft protection passed');
})().catch(e=>{console.error(e);process.exit(1)});
