#!/usr/bin/env python3
"""minisign signature checks of verify-release.py, with a throwaway key made in a temp dir.

Needs the `cryptography` package only to *sign* the fixtures; verification itself is
standard library (and is exercised both with and without cryptography installed).
"""
import base64,builtins,hashlib,importlib.util,io,json,os,pathlib,subprocess,sys,tarfile,tempfile,unittest
HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('verify',HERE/'verify-release.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
try:
 from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
 from cryptography.hazmat.primitives import serialization
except ImportError:Ed25519PrivateKey=None
REQUIRED=['nabiman-server','nabiman-privileged-helper','nabiman-recovery','management-recovery.py','verify-release.py','static/index.html','static/release-inventory.json','static/release-bom.cdx.json','LICENSE','NOTICE','THIRD_PARTY_LICENSES.md','VERSION','nabiman.service','packaging/nabiman-update']
def fixture(path):
 contents={'nabiman/'+n:b'fixture' for n in REQUIRED}
 manifest={'schema_version':1,'kind':'unsigned-package-file-manifest','files':{k:{'sha256':hashlib.sha256(b).hexdigest(),'size':len(b),'mode':0o755} for k,b in contents.items()}}
 contents[v.MANIFEST]=json.dumps(manifest).encode()
 with tarfile.open(path,'w:gz') as f:
  for name,data in contents.items():
   m=tarfile.TarInfo(name);m.size=len(data);m.mode=0o644 if name==v.MANIFEST else 0o755;f.addfile(m,io.BytesIO(data))
class Key:
 """A minisign-format key pair (public key file text + signer) that lives only in memory."""
 def __init__(self):
  self.private=Ed25519PrivateKey.generate();self.key_id=os.urandom(8)
  raw=self.private.public_key().public_bytes(serialization.Encoding.Raw,serialization.PublicFormat.Raw)
  self.public='untrusted comment: throwaway test key\n'+base64.b64encode(b'Ed'+self.key_id+raw).decode()+'\n'
 def sign(self,path,comment,prehash=True):
  data=pathlib.Path(path).read_bytes();message=hashlib.blake2b(data,digest_size=64).digest() if prehash else data
  sig=self.private.sign(message);glob=self.private.sign(sig+comment.encode())
  return 'untrusted comment: test\n'+base64.b64encode((b'ED' if prehash else b'Ed')+self.key_id+sig).decode()+'\ntrusted comment: '+comment+'\n'+base64.b64encode(glob).decode()+'\n'
@unittest.skipIf(Ed25519PrivateKey is None,'cryptography is not installed: cannot make test signatures')
class Signature(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.root=pathlib.Path(self.tmp.name)
  self.archive=self.root/'nabiman-linux-x86_64.tar.gz';fixture(self.archive);self.key=Key()
  self.sig=self.root/'nabiman-linux-x86_64.tar.gz.minisig'
  self.sig.write_text(self.key.sign(self.archive,'version:v9.9.9 file:nabiman-linux-x86_64.tar.gz'))
 def tearDown(self):self.tmp.cleanup()
 def test_valid_signature_authenticates(self):
  result=v.verify_signed(self.archive,self.sig,self.key.public,extract_to=self.root/'out')
  self.assertTrue(result['publisher_authenticated']);self.assertEqual(result['signed_version'],'v9.9.9')
  self.assertTrue((self.root/'out/nabiman/nabiman-server').is_file())
 def test_legacy_unhashed_signature(self):
  self.sig.write_text(self.key.sign(self.archive,'file:nabiman-linux-x86_64.tar.gz',prehash=False))
  self.assertTrue(v.verify_signed(self.archive,self.sig,self.key.public)['publisher_authenticated'])
 def test_standard_library_fallback(self):
  real=builtins.__import__
  def no_cryptography(name,*a,**k):
   if name.startswith('cryptography'):raise ImportError(name)
   return real(name,*a,**k)
  builtins.__import__=no_cryptography
  try:
   self.assertTrue(v.verify_signed(self.archive,self.sig,self.key.public)['publisher_authenticated'])
   self.archive.write_bytes(self.archive.read_bytes()+b'x')
   with self.assertRaises(ValueError):v.verify_signed(self.archive,self.sig,self.key.public)
  finally:builtins.__import__=real
 def test_rejections(self):
  cases={
   'tampered archive':lambda:self.archive.write_bytes(self.archive.read_bytes()+b'\0'),
   'other key':lambda:setattr(self.key,'public',Key().public),
   'other file name':lambda:self.sig.write_text(self.key.sign(self.archive,'version:v1 file:nabiman-linux-aarch64.tar.gz')),
   'no file name':lambda:self.sig.write_text(self.key.sign(self.archive,'version:v1')),
   'edited trusted comment':lambda:self.sig.write_text(self.sig.read_text().replace('v9.9.9','v9.9.10')),
   'truncated':lambda:self.sig.write_text('\n'.join(self.sig.read_text().splitlines()[:2])),
  }
  for name,mutate in cases.items():
   with self.subTest(name):
    self.setUp();mutate()
    with self.assertRaises(ValueError):v.verify_signed(self.archive,self.sig,self.key.public,extract_to=self.root/'out')
    self.assertFalse((self.root/'out').exists());self.tearDown()
 def test_cli_requires_signature_when_asked(self):
  run=lambda *args:subprocess.run([sys.executable,str(HERE/'verify-release.py'),str(self.archive),*args],capture_output=True,text=True)
  self.assertNotEqual(run('--require-signature').returncode,0)
  keyfile=self.root/'release.pub';keyfile.write_text(self.key.public)
  ok=run('--public-key-file',str(keyfile));self.assertEqual(ok.returncode,0,ok.stderr);self.assertTrue(json.loads(ok.stdout)['publisher_authenticated'])
  self.sig.unlink();self.assertNotEqual(run('--public-key-file',str(keyfile)).returncode,0)
 def test_installer_signature_step(self):
  text=(HERE.parent/'install.sh').read_text();code=text.split("<<'PYVERIFY'\n",1)[1].split('\nPYVERIFY\n',1)[0]
  run=lambda key:subprocess.run([sys.executable,'-',str(self.archive),str(self.sig),self.archive.name],input=code,env=dict(os.environ,NABIMAN_RELEASE_PUBKEY=key),capture_output=True,text=True)
  self.assertEqual(run(self.key.public.splitlines()[1]).returncode,0)
  self.assertNotEqual(run(Key().public.splitlines()[1]).returncode,0)
  self.sig.unlink();self.assertNotEqual(run(self.key.public.splitlines()[1]).returncode,0)
 def test_installer_embeds_the_same_verifier(self):
  check=subprocess.run([sys.executable,str(HERE/'check-release-key.py'),'--allow-placeholder'],capture_output=True,text=True)
  self.assertEqual(check.returncode,0,check.stderr)
if __name__=='__main__':unittest.main()
