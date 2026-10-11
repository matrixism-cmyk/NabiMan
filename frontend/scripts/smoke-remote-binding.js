const fs = require('fs');
const { chromium } = require('playwright');
(async () => {
 const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
 const browser = await chromium.launch({headless:true,args:['--no-sandbox']});
 try {
  const page = await browser.newPage(); await page.goto(fixture.base);
  await page.evaluate(async f => {
   const check = (ok, message) => { if (!ok) throw new Error(message); };
   const api = async (path, method='GET', body) => { const r = await fetch(path,{method,headers:{Authorization:'Bearer '+f.token,'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});return {status:r.status,value:await r.json()}; };
   const connect = async (query, shared) => {
    const r = shared ? await fetch('/api/share/'+shared+'/auth',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({password:'',query:''})}) : null;
    const admission = shared ? await r.json() : (await api('/api/terminal/tickets','POST',{query})).value;
    const ws = new WebSocket(location.origin.replace(/^http/,'ws')+(shared?'/api/share/'+shared+'/ws?':'/api/terminal?'+query+'&')+'ticket='+admission.data.ticket);
    const state = {ws,text:'',closed:null,session:null}; ws.binaryType='arraybuffer';
    ws.onmessage=e=>{const data=typeof e.data==='string'?e.data:new TextDecoder().decode(e.data);state.text+=data;if(data.startsWith('\x02SESSION:'))state.session=data.slice(9);};
    ws.onclose=e=>state.closed=e.code;
    await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=()=>reject(new Error('Socket failed'));});
    return state;
   };
   const wait = async (condition, message) => {for(let i=0;i<200;i++){if(condition())return;await new Promise(r=>setTimeout(r,50));}throw new Error(message);};
   if (f.resume) {
    const old = await connect('session_id='+f.resume.old+'&join=1');
    await wait(()=>old.text.includes('\x02ENDED'),'Restart resurrected old approval');
    check(!old.text.includes('REMOTE-READY'),'Restart leaked revoked remote screen');
    const current = await connect('session_id='+f.resume.current+'&join=1');
    await wait(()=>current.text.includes('REMOTE-READY'),'Restart lost current approved attachment');
    check((await api('/api/remote-servers/'+f.server_id,'DELETE')).value.success,'Fixture server deletion failed');
    await wait(()=>current.closed===1008,'Deleted target kept live attachment');
    return;
   }
   const remote = await connect('server_id='+f.server_id);
   try { await wait(()=>remote.session&&remote.text.includes('REMOTE-READY'),'Verified SSH shell did not appear'); } catch(e) { throw new Error(e.message+' '+JSON.stringify(remote.text.slice(-1800))); }
   const local = await connect('cols=80&rows=24'); await wait(()=>local.session,'Local session missing');
   const share = (await api('/api/terminal/sessions/'+remote.session+'/share-link','POST',{expires_in_secs:3600,read_only:true})).value.data.token;
   const guest = await connect('',share); await wait(()=>guest.text.includes('REMOTE-READY'),'Shared remote output missing');
   remote.ws.send('PROBE-BEFORE\n'); await wait(()=>guest.text.includes('PROBE-BEFORE'),'Actual SSH input/output failed');
   const before = (await api('/api/remote-servers')).value.data.find(s=>s.id===f.server_id);
   const rotated = await api('/api/remote-servers/'+f.server_id+'/host-trust','PUT',{host:before.host,port:before.port,public_key:before.host_trust.public_key,fingerprint:before.host_trust.fingerprint,previous_fingerprint:before.host_trust.fingerprint,previous_version:before.host_trust_version,reason:'Reapprove isolated console key'});
   check(rotated.value.success&&rotated.value.data.host_trust_version!==before.host_trust_version,'Approval did not rotate');
   await wait(()=>remote.closed===1008&&guest.closed===1008,'Owner/shared live attachment did not close with 1008');
   check(local.closed===null,'Unrelated local shell closed');
   const oldText = guest.text;
   const resumed = await connect('session_id='+remote.session+'&join=1');
   await wait(()=>resumed.text.includes('\x02ENDED'),'Old approval reattached');
   check(!resumed.text.includes('REMOTE-READY'),'Old remote screen leaked on reconnect');
   const inventory = (await api('/api/terminal/sessions')).value.data.find(s=>s.id===remote.session);
   check(inventory&&!inventory.alive&&!inventory.preview,'Revoked terminal preview leaked');
   const history = await api('/api/terminal/sessions/'+remote.session+'/scrollback');
   check(!history.value.success,'Revoked scrollback remained accessible');
   check(guest.text===oldText,'Revoked shared attachment received output');
   const fresh = await connect('server_id='+f.server_id);
   await wait(()=>fresh.text.includes('REMOTE-READY'),'New approval cannot open new SSH session');
   fresh.ws.close();local.ws.close();
  }, fixture);
  console.log('PASS: real loopback SSH owner/shared input-output, same-key reapproval closes both with 1008, reconnect/history/preview denial, fresh approved session and unrelated local shell preservation.');
 } finally {await browser.close();}
})().catch(e=>{console.error('FAIL: remote binding proof: '+e.message.split('\n')[0]);process.exit(1);});
