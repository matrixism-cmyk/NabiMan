#!/usr/bin/env python3
"""Restore the stopped cutover snapshot into an isolated runtime, without activating it."""
import argparse,datetime,importlib.util,json,os,pathlib,secrets,socket,subprocess,tempfile,time
spec=importlib.util.spec_from_file_location('deployment',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'));d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--deployment',type=pathlib.Path,required=True);args=parser.parse_args()
assert os.geteuid()==0 and args.deployment.parent==d.ROOT/'deployments';os.umask(0o077)
plan=d.read(args.deployment/'plan.json');assert d.read(args.deployment/'activated.json')['deployed'];d.verify_artifacts(args.deployment,plan)
assert d.sha(d.ROOT/'bin/nabiman-recovery')==plan['recovery_sha256'];assert d.sha(d.ROOT/'bin/management-recovery.py')==plan['recovery_script_sha256']
root=args.deployment/'management-recovery';root.mkdir(mode=0o700)
key=d.ROOT/'secrets/management-recovery-passphrase'
if not key.exists():
 fd=os.open(key,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'w') as f:f.write(secrets.token_urlsafe(48));f.flush();os.fsync(f.fileno())
assert key.stat().st_mode&0o077==0
password=key.read_text()
def recover(arguments):
 result=subprocess.run(['python3',str(d.ROOT/'bin/management-recovery.py'),'--passphrase-fd','0',*map(str,arguments)],input=password+'\n',text=True,capture_output=True,timeout=600)
 assert result.returncode==0,'Offline recovery failed: '+result.stderr[-600:]
 return json.loads(result.stdout)
before=args.deployment/'cutover.before';started=time.monotonic()
captured=recover(['capture','--auth',before/'auth','--database',before/'mec.db','--runtime',before/'nabiman-server','--static',before/'static','--output',root/'archives'])
restored=root/'restored';verified=recover(['restore','--archive',captured['archive'],'--target',restored])
assert verified['runtime_sha256']==d.sha(before/'nabiman-server')
for name in ['users.json','jwt_secret','secret.key','remote_servers.json','terminal_sessions.json','terminal_settings.json','terminal_shares.json']:
 assert (before/'auth'/name).read_bytes()==(restored/'auth'/name).read_bytes(),'Restored managed identity or key changed'
for name in ['backup_schedules.json','alert_rules.json']:
 path=restored/'auth'/name
 assert not path.exists() or d.read(path)==[],'Automatic actions require a separate quarantined drill; refusing to start them'
legacy=restored/'auth/audit_forwarding.json'
assert not legacy.exists() or all(not row.get('enabled') for row in d.read(legacy)), 'Legacy forwarding requires separate quarantine'
# This gate is deliberate: a restored runtime is never started with unknown scheduled actions.
with tempfile.TemporaryDirectory(prefix='nb-dr-') as temporary:
 temp=pathlib.Path(temporary);(temp/'tmux').mkdir(mode=0o700)
 env={k:v for k,v in os.environ.items() if not k.startswith('NABIMAN_') and k not in ['TMUX','SSH_AUTH_SOCK','KUBECONFIG']}
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 env.update(NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(restored/'auth'),NABIMAN_MEC_DB=str(restored/'database/mec.db'),NABIMAN_STATIC=str(restored/'runtime/static'),NABIMAN_MEC_MODE='mock',NABIMAN_AUDIT_FORWARDING_DISABLED='1', NABIMAN_API_TOKENS_DISABLED='1',TMUX_TMPDIR=str(temp/'tmux'),TMPDIR=str(temp))
 names=[value['identity']['tmux_name'] for value in plan['sessions'].values()];process=None
 try:
  for name in names:subprocess.run(['tmux','new-session','-d','-s',name,'sleep 3600'],env=env,check=True,capture_output=True)
  with (root/'restored-runtime.log').open('w') as log:process=subprocess.Popen([str(restored/'runtime/nabiman-server')],cwd=restored,env=env,stdout=log,stderr=log)
  base=f'http://127.0.0.1:{port}'
  for _ in range(300):
   assert process.poll() is None,'Restored runtime exited; inspect private drill log'
   try:
    with d.urlopen(base+'/',timeout=1) as response:
     if response.status==200:break
   except OSError:pass
   time.sleep(.1)
  else:raise AssertionError('Restored runtime startup timeout')
  snapshot_plan=dict(plan,binary_sha256=verified['runtime_sha256']);identity=d.check(base,snapshot_plan)
  assert d.tmux_names(env)==sorted(names)
  proof={'passed':True,'verified_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'snapshot_id':verified['snapshot_id'],'archive_sha256':captured['sha256'],'data_verification':verified,'actual_restored_login_and_management_reads':True,'credential_key_bytes_preserved':True,'terminal_inventory_fixture_count':len(names),'isolated_tmux':True,'external_providers':'mock; real remote connectivity not tested','original_runtime_hash':identity['executable_sha256'],'elapsed_seconds':round(time.monotonic()-started,3),'activated_restored_instance':False,'limitations':['This cutover snapshot predates the newly activated release','No enabled backup schedules or alert rules in this drill','Same-host isolated runtime, not off-site or full-host disaster recovery','Recovery passphrase remains local; separate secure off-site key retention required']}
 finally:
  if process and process.poll() is None:
   process.terminate()
   try:process.wait(timeout=12)
   except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
  for name in names:subprocess.run(['tmux','kill-session','-t','='+name],env=env,capture_output=True)
assert set(d.read(before/'tmux.json')).issubset(d.tmux_names()),'Original terminal disappeared'
assert d.check(d.PUBLIC,plan)['executable_sha256']==plan['binary_sha256'],'Production instance changed during restore drill'
proof['production_unchanged_after_cleanup']=True
d.write(args.deployment/'management-recovery-verification.json',proof);print(json.dumps(proof))
