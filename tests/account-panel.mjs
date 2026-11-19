// Run with node --test tests/account-panel.mjs. Execute the QML controllers'
// actual JS, with the transport, timers and model substituted (not a visual test).
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

function controller(file, state) {
  const context = vm.createContext(state);
  const source = fs.readFileSync(new URL('../' + file, import.meta.url), 'utf8');
  for (const [fn] of source.matchAll(/^  function \w+\([^\n]*\) \{[\s\S]*?^  \}/gm)) vm.runInContext(fn, context);
  return context;
}

test('late replies and IDLE are ignored, requests are not queued across a restart', () => {
  let writes = [], events = [], calls = [];
  const b = controller('Backend.qml', {started:true, session:'boot:1', nextId:1, callbacks:{}, proc:{write:s=>writes.push(JSON.parse(s))}});
  b.root = b; b.changed = f => events.push(f);
  const id = b.request('open', {uid:42}, (...args) => calls.push(args));
  b.session = 'boot:2';
  b.handleLine(JSON.stringify({id,ok:true,data:'old message'}));
  for (const session of ['boot:1','boot:2']) b.handleLine(JSON.stringify({event:'changed',folder:'INBOX',session}));
  assert.equal(calls.length,0); assert.deepEqual(events,['INBOX']);
  b.started = false; b.request('send',{},ok=>calls.push(ok));
  assert.deepEqual(calls,[false]); assert.equal(writes.length,1); assert.equal(writes[0].session,'boot:1');
});

function panel() {
  const requests = [];
  const p = controller('Panel.qml', {busy:false, confirming:false, sending:false, configured:true, phase:'mail',
    accounts:[{id:'A',email:'a@example.test'},{id:'B',email:'b@example.test'}],
    accountId:'A', email:'a@example.test', draft:{owner:'A',from:'a@example.test'},
    backend:{session:'one',request:(cmd,args,cb)=>requests.push({cmd,args,cb})},
    pollTimer:{restart(){}},openTimer:{stop(){}},statusTimer:{stop(){},restart(){}},messages:{count:0,clear(){}},folders:[1],listToken:4,viewToken:3,
    Qt:{callLater(){}}, listLoader:{item:null},confirmDialog:{},keySink:{forceActiveFocus(){}}, composerLoader:{item:{focusFirst(){}}}});
  p.mailApp=p; return {p,requests};
}

test('cancel or failed switch preserves draft and session; successful switch clears mailbox state', () => {
  const {p,requests}=panel();
  Object.assign(p,{folder:'Sent',query:'old',view:{uid:42},undoId:9,refreshPending:true,pendingSelectUid:42});
  p.changeAccount('select',{account:'B'},true); assert.equal(requests.length,0); p.finishConfirm(false);
  assert.equal(p.draft.owner,'A'); assert.equal(requests.length,0);
  p.changeAccount('select',{account:'B'},true); p.finishConfirm(true); requests.pop().cb(false,'auth error');
  assert.equal(p.view.uid,42); assert.equal(p.draft.owner,'A'); assert.equal(p.backend.session,'one');
  p.changeAccount('select',{account:'B'},true); p.finishConfirm(true);
  requests.pop().cb(true,{configured:true,accounts:p.accounts,email:p.accounts[1].email,account:'B',session:'two'});
  assert.equal(p.folder,'INBOX'); assert.equal(p.query,''); assert.equal(p.undoId,0); assert.equal(p.view,null);
  assert.equal(p.refreshPending,false); assert.equal(p.pendingSelectUid,-1); assert.equal(p.draft,null);
  assert.equal(p.email,'b@example.test'); assert.equal(p.backend.session,'two');
});

test('notification actions never fall back to the active mailbox', () => {
  const {p,requests}=panel(); p.draft=null;
  for (const account of [undefined,'removed']) p.routeTarget({account,folder:'INBOX',uid:42,uidvalidity:7});
  assert.equal(requests.length,0);
  p.routeTarget({account:'B',folder:'INBOX',uid:42,uidvalidity:7});
  const select=requests.pop(); assert.equal(select.cmd,'select');assert.equal(select.args.account,'B');
  select.cb(true,{configured:true,accounts:p.accounts,email:p.accounts[1].email,account:'B',session:'two'});
  assert.deepEqual(requests.map(r=>r.cmd),['open','cached','list','folders']);
  assert.equal(requests[0].args.uid,42); assert.equal(requests[0].args.uidvalidity,7);
});

test('opening from a notification starts with the message, not the folder counts', () => {
  const {p,requests}=panel(); p.draft=null;
  p.pendingTarget={account:'A',folder:'INBOX',uid:42,uidvalidity:7};
  p.start(); const status=requests.shift(); assert.equal(status.cmd,'status');
  status.cb(true,{configured:true,accounts:p.accounts,email:p.accounts[0].email,account:'A',session:'two'});
  assert.deepEqual(requests.map(r=>r.cmd),['open','cached','list','folders']);
  assert.equal(p.selectedUid,42);
});

test('a failed notification-driven switch at startup loads the active account', () => {
  const {p,requests}=panel(); p.draft=null;
  p.pendingTarget={account:'B',folder:'INBOX',uid:42,uidvalidity:7};
  p.start();
  requests.shift().cb(true,{configured:true,accounts:p.accounts,email:p.accounts[0].email,account:'A',session:'two'});
  assert.deepEqual(requests.map(r=>r.cmd),['select']);
  requests.shift().cb(false,'Password rejected');
  assert.deepEqual(requests.map(r=>r.cmd),['cached','list','folders']);
  assert.equal(p.accountId,'A');
});

test('an in-flight send blocks account actions; an old draft cannot be sent as another account', () => {
  const {p,requests}=panel(); p.sending=true;
  p.changeAccount('remove',{account:'A'},true);p.routeTarget({account:'B',folder:'INBOX',uid:42}); p.manageAccounts();
  assert.equal(p.phase,'mail');assert.equal(requests.length,0);assert.equal(p.confirming,false);
  p.sending=false;p.accountId='B';p.sendDraft({to:'recipient@example.test'});
  assert.equal(requests.length,0);assert.match(p.sendError,/draft/);
});
