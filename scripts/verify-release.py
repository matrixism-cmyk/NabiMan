#!/usr/bin/env python3
"""Verify every regular file against the package manifest before optional new-directory extraction.

This detects inconsistent package contents, not publisher identity. Verify a trusted external
archive digest/signature first. No code from the archive is executed by this verifier.
"""
import argparse,ctypes,hashlib,json,os,pathlib,shutil,tarfile,tempfile
MAX_BYTES=2*1024*1024*1024
MAX_FILES=20000
MANIFEST='nabiman/release-manifest.json'
def safe_name(name):
 p=pathlib.PurePosixPath(name)
 return name==p.as_posix() and not p.is_absolute() and '..' not in p.parts and not any(ord(c)<32 or ord(c)==127 for c in name) and (name=='nabiman' or name.startswith('nabiman/'))
def verify(archive,extract_to=None):
 with tarfile.open(archive,'r:gz') as package:
  members=[];names=set();total=0
  for member in package:
   if len(members)>=MAX_FILES or not safe_name(member.name) or member.name in names:raise ValueError('Unsafe, duplicate or excessive archive paths')
   if not (member.isfile() or member.isdir()) or member.sparse is not None:raise ValueError('Links, sparse and special files are forbidden')
   if member.mode & 0o7000 or member.size<0:raise ValueError('Privileged mode or invalid size')
   if member.isdir() and member.size:raise ValueError('Directory with payload')
   total+=member.size
   if total>MAX_BYTES:raise ValueError('Expanded package exceeds limit')
   names.add(member.name);members.append(member)
  by_name={m.name:m for m in members};manifest=by_name.get(MANIFEST)
  if not manifest or not manifest.isfile() or manifest.size>8*1024*1024:raise ValueError('Missing or oversized manifest')
  with package.extractfile(manifest) as stream:manifest_bytes=stream.read()
  receipt=json.loads(manifest_bytes)
  if receipt.get('schema_version')!=1 or receipt.get('kind')!='unsigned-package-file-manifest':raise ValueError('Unsupported manifest')
  expected=receipt['files']
  regular={m.name:m for m in members if m.isfile() and m.name!=MANIFEST}
  if set(expected)!=set(regular):raise ValueError('Manifest file set differs from package')
  required={'nabiman/nabiman-server','nabiman/nabiman-privileged-helper','nabiman/nabiman-recovery','nabiman/management-recovery.py','nabiman/verify-release.py','nabiman/static/index.html','nabiman/static/release-inventory.json','nabiman/static/release-bom.cdx.json'}
  if not required.issubset(regular):raise ValueError('Incomplete release')
  for name,member in regular.items():
   entry=expected[name]
   if member.size!=entry['size'] or member.mode&0o777!=entry['mode']:raise ValueError('File size or mode mismatch')
   digest=hashlib.sha256()
   with package.extractfile(member) as stream:
    while True:
     chunk=stream.read(65536)
     if not chunk:break
     digest.update(chunk)
   if digest.hexdigest()!=entry['sha256']:raise ValueError('File digest mismatch')
  # Parent/file collisions are invalid even if each leaf digest is consistent.
  for name in names:
   for parent in pathlib.PurePosixPath(name).parents:
    if parent.as_posix() in by_name and not by_name[parent.as_posix()].isdir():raise ValueError('File used as parent')
  if extract_to:
   target=pathlib.Path(extract_to)
   if target.exists() or target.is_symlink():raise ValueError('Extraction target already exists')
   stage=pathlib.Path(tempfile.mkdtemp(prefix='.nabiman-extract-',dir=target.parent))
   try:
    for member in members:
     destination=stage/member.name
     if member.isdir():destination.mkdir(parents=True,exist_ok=True);continue
     destination.parent.mkdir(parents=True,exist_ok=True)
     with package.extractfile(member) as source,destination.open('xb') as output:shutil.copyfileobj(source,output)
     digest=hashlib.sha256()
     with destination.open('rb') as extracted:
      while True:
       chunk=extracted.read(65536)
       if not chunk:break
       digest.update(chunk)
     expected_hash=hashlib.sha256(manifest_bytes).hexdigest() if member.name==MANIFEST else expected[member.name]['sha256']
     if digest.hexdigest()!=expected_hash:raise ValueError('Archive changed during extraction')
     destination.chmod(member.mode&0o777)
    # Linux release packages: atomic no-replace publication, including empty directories.
    libc=ctypes.CDLL(None,use_errno=True)
    if not hasattr(libc,'renameat2'):raise ValueError('Atomic no-replace extraction requires Linux renameat2')
    if libc.renameat2(-100,os.fsencode(stage),-100,os.fsencode(target),1)!=0:
     raise OSError(ctypes.get_errno(),'Cannot publish extraction without replacing target')
   finally:
    if stage.exists():shutil.rmtree(stage)
  return {'verified':True,'files':len(regular),'expanded_bytes':total,'publisher_authenticated':False}
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('archive',type=pathlib.Path);parser.add_argument('--extract-to',type=pathlib.Path);a=parser.parse_args()
 try:print(json.dumps(verify(a.archive,a.extract_to)))
 except (ValueError,KeyError,TypeError,OSError,tarfile.TarError) as error:raise SystemExit('Release verification failed: '+str(error))
