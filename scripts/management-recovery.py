#!/usr/bin/env python3
"""Capture stopped NabiMan managed state, or verify/restore an authenticated bundle.
No service is stopped or started by this tool. No recovery is applied over live paths.
"""
import argparse,contextlib,ctypes,datetime,fcntl,getpass,hashlib,json,os,pathlib,sqlite3,stat,subprocess,tempfile,time,uuid
MAX_BYTES=1024*1024*1024
EXCLUDED=['runtime lock files','backup archives and crash staging','temporary SSH password files','live tmux processes and in-memory login sessions','OS SSH identities/agents outside managed auth storage','external provider configuration/environment/kubeconfig','reverse proxy, TLS and operating system configuration','remote server disks and external databases']
def digest(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for data in iter(lambda:f.read(1024*1024),b''):h.update(data)
 return h.hexdigest()
def private_write(path,data):
 with os.fdopen(os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),'wb') as f:f.write(data);f.flush();os.fsync(f.fileno())
def secret(args):
 if args.passphrase_fd is None:
  value=getpass.getpass('Recovery bundle passphrase (not stored): ')
  if args.action=='capture' and value!=getpass.getpass('Confirm recovery passphrase: '):raise ValueError('Passphrases do not match')
 else:
  with os.fdopen(os.dup(args.passphrase_fd),'rb') as stream:value=stream.read(1026).decode().rstrip('\r\n')
 if len(value)<12 or len(value.encode())>1024 or any(ord(c)<32 or ord(c)==127 for c in value):raise ValueError('Use a passphrase of at least 12 characters, at most 1024 bytes')
 return value
@contextlib.contextmanager
def ownership(auth,db):
 with contextlib.ExitStack() as stack:
  for path,create in [(db,False)]+[(auth/name,True) for name in ['terminal_shares.lock','terminal_sessions.lock','terminal_settings.lock']]:
   flags=os.O_RDWR|os.O_NOFOLLOW|os.O_NONBLOCK|(os.O_CREAT if create else 0)
   file=stack.enter_context(os.fdopen(os.open(path,flags,0o600),'r+b'))
   if not stat.S_ISREG(os.fstat(file.fileno()).st_mode):raise ValueError('Ownership path is not a regular file')
   try:fcntl.flock(file,fcntl.LOCK_EX|fcntl.LOCK_NB)
   except BlockingIOError:raise ValueError('NabiMan still owns the data; stop the server before capturing') from None
  yield
class Copy:
 def __init__(self,root):self.root=root;self.files={};self.bytes=0
 def file(self,source,name,mode=0o600):
  if len(self.files)>=100000:raise ValueError('Bundle entry limit exceeded')
  before=source.lstat()
  if not stat.S_ISREG(before.st_mode):raise ValueError('Bundle source contains a symlink or special file')
  self.bytes+=before.st_size
  if self.bytes>MAX_BYTES-1024*1024:raise ValueError('Bundle exceeds size limit')
  target=self.root/name;target.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
  fd=os.open(source,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK)
  with os.fdopen(fd,'rb') as src,os.fdopen(os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL,mode),'wb') as dst:
   opened=os.fstat(src.fileno())
   if not stat.S_ISREG(opened.st_mode) or (opened.st_dev,opened.st_ino)!=(before.st_dev,before.st_ino):raise ValueError('Bundle source changed')
   copied=0
   for data in iter(lambda:src.read(1024*1024),b''):
    copied+=len(data)
    if copied>before.st_size:raise ValueError('Bundle source grew during capture')
    dst.write(data)
   after=os.fstat(src.fileno())
   if copied!=before.st_size or after.st_mtime_ns!=before.st_mtime_ns:raise ValueError('Bundle source changed during capture')
   dst.flush();os.fsync(dst.fileno())
  self.files[name]={'sha256':digest(target),'size':target.stat().st_size,'mode':mode}
 def tree(self,source,prefix,auth=False,depth=0):
  if depth>64:raise ValueError('Bundle source depth exceeded')
  for item in sorted(source.iterdir()):
   relative=prefix+'/'+item.name
   if auth and (item.name.endswith('.lock') or relative=='auth/backups' or item.name.startswith('.private-backup-') or item.name.startswith('.pending-') or relative=='auth/ssh/pw'):continue
   mode=item.lstat().st_mode
   if stat.S_ISDIR(mode):self.tree(item,relative,auth,depth+1)
   elif stat.S_ISREG(mode):self.file(item,relative)
   else:raise ValueError('Bundle source contains a symlink or special file')
def call(tool,action,args,password=None):
 result=subprocess.run([str(tool),action,*map(str,args)],input=None if password is None else password+'\n',text=True,capture_output=True,timeout=600)
 if result.returncode:raise ValueError(result.stderr.strip()[:400] or 'Recovery tool failed')
 return json.loads(result.stdout)
def capture(args,password):
 auth=args.auth.resolve(strict=True);db=args.database.resolve(strict=True);runtime=args.runtime.resolve(strict=True);static=args.static.resolve(strict=True)
 output=args.output.resolve();output.mkdir(mode=0o700,parents=True,exist_ok=True)
 if output.stat().st_uid!=os.geteuid() or output.stat().st_mode&0o077:raise ValueError('Recovery output must be an owned private directory (0700)')
 if output==auth or auth in output.parents or output==static or static in output.parents:raise ValueError('Recovery output must be outside captured trees')
 with ownership(auth,db),tempfile.TemporaryDirectory(prefix='.management-capture-',dir=output) as temporary:
  root=pathlib.Path(temporary);copy=Copy(root)
  copy.tree(auth,'auth',True)
  copy.file(runtime,'runtime/nabiman-server',0o700);copy.tree(static,'runtime/static')
  (root/'database').mkdir(mode=0o700)
  target=root/'database/mec.db'
  fd=os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600);os.close(fd)
  with sqlite3.connect(db.as_uri()+'?mode=ro',uri=True) as source,sqlite3.connect(target) as dest:
   page_size=source.execute('PRAGMA page_size').fetchone()[0]
   def progress(status,remaining,total):
    if total*page_size+copy.bytes>MAX_BYTES-1024*1024:raise ValueError('Database exceeds remaining bundle size limit')
   source.backup(dest,pages=128,progress=progress)
  with target.open('rb') as stream:os.fsync(stream.fileno())
  copy.files['database/mec.db']={'sha256':digest(target),'size':target.stat().st_size,'mode':0o600}
  manifest={'schema_version':1,'kind':'nabiman-management-state','snapshot_id':str(uuid.uuid4()),'created_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'files':copy.files,'excluded':EXCLUDED}
  private_write(root/'manifest.json',json.dumps(manifest,ensure_ascii=False,indent=2).encode())
  verified=call(args.tool,'verify',[root]);packed=call(args.tool,'pack',[root,output],password)
  archive=output/packed['filename'];proof={'snapshot_id':manifest['snapshot_id'],'archive':str(archive),'sha256':digest(archive),'verification':verified,'capture_seconds':round(time.monotonic()-args.started,3)}
  private_write(output/(archive.name+'.json'),json.dumps(proof,ensure_ascii=False,indent=2).encode());return proof

def main():
 if ctypes.CDLL(None).prctl(4,0,0,0,0)!=0:raise ValueError('Cannot protect recovery process')
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--tool',type=pathlib.Path,default=pathlib.Path(__file__).resolve().parent/'nabiman-recovery')
 parser.add_argument('--passphrase-fd',type=int,help='Read passphrase from this already-open descriptor; never put it in argv or environment')
 sub=parser.add_subparsers(dest='action',required=True)
 p=sub.add_parser('capture')
 for name in ['auth','database','runtime','static','output']:p.add_argument('--'+name,type=pathlib.Path,required=True)
 p=sub.add_parser('restore');p.add_argument('--archive',type=pathlib.Path,required=True);p.add_argument('--target',type=pathlib.Path,required=True)
 p=sub.add_parser('verify');p.add_argument('--target',type=pathlib.Path,required=True)
 args=parser.parse_args();args.tool=args.tool.resolve(strict=True);args.started=time.monotonic();os.umask(0o077)
 if args.action=='verify':result=call(args.tool,'verify',[args.target.resolve(strict=True)])
 else:
  password=secret(args)
  if args.action=='capture':result=capture(args,password)
  else:result=call(args.tool,'restore',[args.archive.resolve(strict=True),args.target.absolute()],password)
 print(json.dumps(result,ensure_ascii=False))
if __name__=='__main__':
 try:main()
 except (OSError,ValueError,sqlite3.Error,subprocess.SubprocessError) as error:raise SystemExit('Management recovery failed: '+str(error))
