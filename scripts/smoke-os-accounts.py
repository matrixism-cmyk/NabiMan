#!/usr/bin/env python3
"""Real release + shadow/PAM tools inside a private chroot; no host OS account mutation."""
import ctypes,hashlib,http.client,json,os,pathlib,re,shutil,socket,stat,subprocess,tempfile,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/release/nabiman-server')).resolve()
assert os.geteuid()==0,'This isolated chroot fixture requires the authorized local test operator'
os.umask(0o077)
host_files=[pathlib.Path('/etc')/n for n in ['passwd','shadow','group','gshadow']]
before={p:hashlib.sha256(p.read_bytes()).hexdigest() for p in host_files}
def cron_hashes():
 return {str(p):hashlib.sha256(p.read_bytes()).hexdigest() for folder in ['/var/spool/cron/crontabs','/var/spool/cron'] if pathlib.Path(folder).exists() for p in pathlib.Path(folder).iterdir() if p.is_file()}
cron_before=cron_hashes()
with tempfile.TemporaryDirectory(prefix='nb-os-account-root-') as temporary:
 root=pathlib.Path(temporary);installed=set()
 # Merged-/usr layout, like current hosts: libpam looks for modules under /lib on
 # Ubuntu 22.04 and under /usr/lib on 24.04, and both must reach the copy.
 for name in ['bin','sbin','lib','lib64']:(root/'usr'/name).mkdir(parents=True);(root/name).symlink_to('usr/'+name)
 def copy_binary(source,destination=None):
  source=pathlib.Path(source);destination=destination or str(source)
  target=root/destination.lstrip('/');target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target);target.chmod(0o755)
  if str(source) in installed:return
  installed.add(str(source))
  output=subprocess.run(['ldd',str(source)],text=True,capture_output=True,check=True).stdout
  for name in re.findall(r'(?:=>\s+)?(/[^\s]+)',output):
   p=pathlib.Path(name)
   if p.is_file():
    t=root/str(p).lstrip('/');t.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,t);t.chmod(0o755)
 for name in ['/usr/sbin/useradd','/usr/sbin/userdel','/usr/sbin/chpasswd','/usr/bin/crontab','/bin/sh','/bin/bash','/lib/x86_64-linux-gnu/security/pam_unix.so','/sbin/unix_chkpwd','/sbin/unix_update']:
  original=pathlib.Path(name+'.distrib')
  copy_binary(original if original.is_file() else name,name)
 copy_binary(BINARY,'/usr/local/bin/nabiman-server')
 for name in ['etc/pam.d','etc/default','home','root','tmp','dev','var/mail','var/lib/nabiman','proc/self','run','var/spool/cron/crontabs']:(root/name).mkdir(parents=True,exist_ok=True)
 configs={'passwd':'root:x:0:0::/root:/bin/bash\nfixture_seed:x:60000:60000::/nonexistent:/bin/sh\n','group':'root:x:0:\ncrontab:x:101:\nfixture_seed:x:60000:\n','shadow':'root:!:20000:0:99999:7:::\nfixture_seed:!:20000:0:99999:7:::\n','gshadow':'root:!::\nfixture_seed:!::\n','shells':'/bin/bash\n/bin/sh\n','nsswitch.conf':'passwd: files\ngroup: files\nshadow: files\nhosts: files\n','hosts':'127.0.0.1 localhost\n','login.defs':'UID_MIN 1000\nUID_MAX 65533\nGID_MIN 1000\nGID_MAX 65533\nUSERGROUPS_ENAB yes\nCREATE_HOME yes\nENCRYPT_METHOD SHA512\nSUB_UID_MIN 100000\nSUB_UID_MAX 600100000\nSUB_UID_COUNT 65536\nSUB_GID_MIN 100000\nSUB_GID_MAX 600100000\nSUB_GID_COUNT 65536\n','pam.d/chpasswd':'password required pam_unix.so sha512\n','default/useradd':'GROUP=0\nHOME=/home\nSHELL=/bin/bash\nCREATE_MAIL_SPOOL=no\n','subuid':'','subgid':''}
 for name,text in configs.items():(root/'etc'/name).write_text(text)
 for name,minor in [('null',3),('urandom',9)]:os.mknod(root/'dev'/name,stat.S_IFCHR|0o666,os.makedev(1,minor))
 # Mount namespaces are unavailable here. This fixture supplies only the executable
 # link needed for startup; it is not a kernel /proc or runtime-identity conformance test.
 (root/'proc/self/exe').symlink_to('/usr/local/bin/nabiman-server')
 shutil.copytree(ROOT/'frontend/build',root/'static')
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}';password='isolated-os-fixture-password'
 env={k:v for k,v in os.environ.items() if not k.startswith('NABIMAN_') and k not in ['TMUX','SSH_AUTH_SOCK','KUBECONFIG']};env.update(NABIMAN_PORT=str(port),NABIMAN_DATA_DIR='/var/lib/nabiman',NABIMAN_MEC_DB='/var/lib/nabiman/mec.db',NABIMAN_MEC_MODE='mock',NABIMAN_STATIC='/static',NABIMAN_PASSWORD=password,TMUX_TMPDIR='/tmp')
 def call(path,body=None,token=None,method=None):
  c=http.client.HTTPConnection('127.0.0.1',port,timeout=40)
  try:
   headers={'Content-Type':'application/json'}
   if token:headers['Authorization']='Bearer '+token
   c.request(method or ('GET' if body is None else 'POST'),path,body=None if body is None else json.dumps(body),headers=headers)
   r=c.getresponse();raw=r.read();return r.status,json.loads(raw) if raw.startswith(b'{') else None
  finally:c.close()
 with (root/'server.log').open('w') as log:
  p=subprocess.Popen(['/usr/sbin/chroot',str(root),'/usr/local/bin/nabiman-server'],env=env,stdout=log,stderr=log)
  try:
   for _ in range(300):
    assert p.poll() is None,'Chroot fixture exited: '+(root/'server.log').read_text()[-1200:]
    try:
     if call('/')[0]==200:break
    except OSError:pass
    time.sleep(.1)
   else:raise AssertionError('Chroot fixture startup timeout')
   token=call('/api/auth/login',{'username':'admin','password':password})[1]['data']['token']
   initial={name:(root/'etc'/name).read_bytes() for name in ['passwd','shadow','group','gshadow']}
   # Invalid input and stale/protected identities cannot change even fixture OS files.
   for body in [{'username':'-r','new_password':'fixture-password-long','expected_uid':60000},{'username':'fixture_seed','new_password':'fixture-long\nroot:injected','expected_uid':60000},{'username':'root','new_password':'fixture-password-long','expected_uid':0}]:
    assert call('/api/accounts/password',body,token)[0] in [400,409]
   assert all((root/'etc'/n).read_bytes()==data for n,data in initial.items())
   name='nbosfixture';first='fixture-first-password-long';second='fixture-second-password-long'
   code,value=call('/api/accounts',{'username':name,'password':first,'shell':'/bin/bash'},token)
   assert code==200,value

   rows=call('/api/accounts',token=token)[1]['data'];row=next(r for r in rows if r['username']==name);uid=row['uid'];assert row['can_manage'] and uid>=1000 and row['is_logged_in'] is None
   crypt=ctypes.CDLL('libcrypt.so.1');crypt.crypt.argtypes=[ctypes.c_char_p,ctypes.c_char_p];crypt.crypt.restype=ctypes.c_char_p
   def password_matches(value):
    hashed=next(line.split(':')[1] for line in (root/'etc/shadow').read_text().splitlines() if line.split(':')[0]==name)
    return crypt.crypt(value.encode(),hashed.encode())==hashed.encode()
   assert password_matches(first)
   assert call('/api/accounts/password',{'username':name,'expected_uid':uid+1,'new_password':second},token)[0]==409 and password_matches(first)
   assert call('/api/accounts/password',{'username':name,'expected_uid':uid,'new_password':second},token)[0]==200 and password_matches(second)
   home=root/'home'/name;(home/'canary').write_text('preserve')
   assert call('/api/accounts',{'username':name,'expected_uid':uid},token,'DELETE')[0]==200
   assert (home/'canary').read_text()=='preserve'
   # Existing retained homes must not be silently adopted by a newly created user.
   assert call('/api/accounts',{'username':name,'password':first,'shell':'/bin/bash'},token)[0]==409
   name='nbosremove';assert call('/api/accounts',{'username':name,'password':first,'shell':'/bin/bash'},token)[0]==200
   row=next(r for r in call('/api/accounts',token=token)[1]['data'] if r['username']==name)
   assert call('/api/accounts',{'username':name,'expected_uid':row['uid'],'remove_home':True,'confirm_home':'/wrong'},token,'DELETE')[0]==409
   assert call('/api/accounts',{'username':name,'expected_uid':row['uid'],'remove_home':True,'confirm_home':row['home']},token,'DELETE')[0]==200
   assert not (root/'home'/name).exists()
   browser_run=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-os-accounts.js')],input=json.dumps({'base':base,'root':str(root),'token':token,'chromiumPath':os.environ.get('NABIMAN_CHROMIUM','/workspace/nabi/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome')}),text=True,capture_output=True,timeout=70,cwd=ROOT/'frontend')
   assert browser_run.returncode==0,'OS account browser fixture failed; credential-bearing browser logs are not printed'
   browser=json.loads(browser_run.stdout)
   from cron_fixture import verify
   cron=verify(root,call,token,base,ROOT,os.environ.get('NABIMAN_CHROMIUM','/workspace/nabi/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome'))
   # Unreadable/malformed inventory must be 503, not a successful empty list.
   original=(root/'etc/passwd').read_bytes();(root/'etc/passwd').write_text('malformed')
   try:assert call('/api/accounts',token=token)[0]==503
   finally:(root/'etc/passwd').write_bytes(original)
   print(json.dumps({'passed':True,'cron':cron,'browser':browser,'host_crontabs_unchanged':cron_hashes()==cron_before,'real_shadow_tools_and_pam_in_chroot':True,'password_hashes_verified':True,'newline_option_stale_uid_and_root_rejected':True,'home_retained_by_default':True,'explicit_home_removal_verified':True,'retained_home_not_adopted':True,'malformed_inventory_is_503':True,'host_account_files_unchanged':all(hashlib.sha256(p.read_bytes()).hexdigest()==before[p] for p in host_files),'limitations':['Private chroot with minimal files/PAM; not a complete distribution or kernel namespace test','Synthetic executable link only; production runtime identity tested separately','Distribution shadow binaries are tested; host persistence wrappers and external NSS/PAM/deletion hooks are not covered']}))
  finally:
   p.terminate()
   try:p.wait(timeout=10)
   except subprocess.TimeoutExpired:p.kill();p.wait()
assert all(hashlib.sha256(p.read_bytes()).hexdigest()==before[p] for p in host_files),'Host account files changed'
assert cron_hashes()==cron_before,'Host crontabs changed'
