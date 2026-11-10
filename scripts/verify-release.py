#!/usr/bin/env python3
"""Verify a release package: its minisign signature, then every regular file against the
package manifest, before optional new-directory extraction.

With --public-key/--public-key-file and --signature the publisher is authenticated: the
signature must be made by that key over these exact archive bytes, and its trusted comment
must name the archive (file:<name>). The public key has to come from somewhere other than the
release being checked (install.sh embeds it; installs keep it in /etc/nabiman/release.pub;
the repository has docs/release/nabiman-release.pub). Without them only consistency is
checked, and the result says publisher_authenticated: false.
No code from the archive is executed by this verifier. Standard library only.
"""
import argparse,ctypes,hashlib,json,os,pathlib,shutil,tarfile,tempfile
# --- minisign verifier begin (kept identical in install.sh; scripts/test-release-signature.py checks) ---
import base64 as _b64,hashlib as _hl
_P=2**255-19;_L=2**252+27742317777372353535851937790883648493;_D=-121665*pow(121666,_P-2,_P)%_P;_I=pow(2,(_P-1)//4,_P)
def _add(a,b):
 A=(a[1]-a[0])*(b[1]-b[0])%_P;B=(a[1]+a[0])*(b[1]+b[0])%_P;C=2*a[3]*b[3]*_D%_P;E=2*a[2]*b[2]%_P
 e,f,g,h=B-A,E-C,E+C,B+A;return (e*f%_P,g*h%_P,f*g%_P,e*h%_P)
def _mul(s,a):
 q=(0,1,1,0)
 while s:
  if s&1:q=_add(q,a)
  a=_add(a,a);s>>=1
 return q
def _point(raw):
 y=int.from_bytes(raw,'little');sign=y>>255;y&=(1<<255)-1
 if y>=_P:return None
 x2=(y*y-1)*pow(_D*y*y+1,_P-2,_P)%_P
 if x2==0:
  if sign:return None
  return (0,y,1,0)
 x=pow(x2,(_P+3)//8,_P)
 if (x*x-x2)%_P:x=x*_I%_P
 if (x*x-x2)%_P:return None
 if x&1!=sign:x=_P-x
 return (x,y,1,x*y%_P)
_GY=4*pow(5,_P-2,_P)%_P;_G=_point(_GY.to_bytes(32,'little'))
def ed25519_verify(public,message,signature):
 """RFC 8032 Ed25519 verification; uses the cryptography package when it is installed."""
 if len(public)!=32 or len(signature)!=64:return False
 try:
  from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
  from cryptography.exceptions import InvalidSignature
  try:Ed25519PublicKey.from_public_bytes(public).verify(signature,message);return True
  except InvalidSignature:return False
 except ImportError:pass
 a=_point(public);r=_point(signature[:32]);s=int.from_bytes(signature[32:],'little')
 if a is None or r is None or s>=_L:return False
 h=int.from_bytes(_hl.sha512(signature[:32]+public+message).digest(),'little')%_L
 left=_mul(s,_G);right=_add(r,_mul(h,a))
 return (left[0]*right[2]-right[0]*left[2])%_P==0 and (left[1]*right[2]-right[1]*left[2])%_P==0
def minisign_public_key(text):
 """A minisign public key: the base64 line alone or the whole .pub file. Returns (key id, key)."""
 lines=[l.strip() for l in text.strip().splitlines() if l.strip() and not l.startswith('untrusted comment:')]
 if len(lines)!=1:raise ValueError('Public key must be one minisign key')
 raw=_b64.b64decode(lines[0],validate=True)
 if len(raw)!=42 or raw[:2]!=b'Ed':raise ValueError('Not a minisign Ed25519 public key')
 return raw[2:10],raw[10:]
def minisign_verify(path,signature_text,public_key_text):
 """Checks a minisign signature over the file at path; returns its trusted comment."""
 key_id,public=minisign_public_key(public_key_text)
 lines=signature_text.splitlines()
 if len(lines)<4 or not lines[0].startswith('untrusted comment:') or not lines[2].startswith('trusted comment: '):raise ValueError('Malformed minisign signature')
 sig=_b64.b64decode(lines[1].strip(),validate=True);glob=_b64.b64decode(lines[3].strip(),validate=True)
 if len(sig)!=74 or sig[:2] not in (b'Ed',b'ED') or len(glob)!=64:raise ValueError('Unsupported minisign signature')
 if sig[2:10]!=key_id:raise ValueError('Signed by a different key (key id mismatch)')
 if sig[:2]==b'ED':
  digest=_hl.blake2b(digest_size=64)
  with open(path,'rb') as f:
   for chunk in iter(lambda:f.read(1<<20),b''):digest.update(chunk)
  message=digest.digest()
 else:
  with open(path,'rb') as f:message=f.read()
 if not ed25519_verify(public,message,sig[10:]):raise ValueError('Signature does not match the file')
 trusted=lines[2][len('trusted comment: '):]
 if not ed25519_verify(public,sig[10:]+trusted.encode(),glob):raise ValueError('Trusted comment signature is invalid')
 return trusted
# --- minisign verifier end ---
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
  required={'nabiman/nabiman-server','nabiman/nabiman-privileged-helper','nabiman/nabiman-recovery','nabiman/management-recovery.py','nabiman/verify-release.py','nabiman/static/index.html','nabiman/static/release-inventory.json','nabiman/static/release-bom.cdx.json','nabiman/LICENSE','nabiman/NOTICE','nabiman/THIRD_PARTY_LICENSES.md','nabiman/VERSION','nabiman/nabiman.service','nabiman/packaging/nabiman-update'}
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
def trusted_fields(comment):
 """`key:value` words of a trusted comment, e.g. version:v1.2.0 file:nabiman-linux-x86_64.tar.gz."""
 return dict(word.split(':',1) for word in comment.split() if ':' in word)
def verify_signed(archive,signature,public_key_text,expect_file=None,extract_to=None):
 """Signature first (nothing else is read from an unauthenticated archive), then the manifest."""
 comment=minisign_verify(archive,pathlib.Path(signature).read_text(),public_key_text)
 fields=trusted_fields(comment);name=expect_file or pathlib.Path(archive).name
 if fields.get('file')!=name:raise ValueError('Signature is for '+repr(fields.get('file'))+', not '+repr(name))
 result=verify(archive,extract_to);result.update(publisher_authenticated=True,signed_version=fields.get('version'),trusted_comment=comment)
 return result
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter);parser.add_argument('archive',type=pathlib.Path);parser.add_argument('--extract-to',type=pathlib.Path)
 parser.add_argument('--signature',type=pathlib.Path,help='minisign signature (default: ARCHIVE.minisig when a key is given)')
 key=parser.add_mutually_exclusive_group();key.add_argument('--public-key',help='minisign public key (base64 line)');key.add_argument('--public-key-file',type=pathlib.Path)
 parser.add_argument('--expect-file',help='file name the signature must be for (default: the archive name)')
 parser.add_argument('--require-signature',action='store_true',help='fail unless a key and a signature are given and match');a=parser.parse_args()
 try:
  key_text=a.public_key or (a.public_key_file.read_text() if a.public_key_file else None)
  if key_text is None and (a.signature or a.require_signature):raise ValueError('A public key (--public-key or --public-key-file) is required to check a signature')
  if key_text is None:print(json.dumps(verify(a.archive,a.extract_to)))
  else:print(json.dumps(verify_signed(a.archive,a.signature or pathlib.Path(str(a.archive)+'.minisig'),key_text,a.expect_file,a.extract_to)))
 except (ValueError,KeyError,TypeError,OSError,tarfile.TarError) as error:raise SystemExit('Release verification failed: '+str(error))
