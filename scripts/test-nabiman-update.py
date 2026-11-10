#!/usr/bin/env python3
"""packaging/nabiman-update in a throwaway prefix: install, update, automatic rollback,
--rollback, downgrade and signature refusals, legacy adoption and pruning.

The "server" in each fixture package is a tiny Python HTTP server; version "bad" exits at
once, so its health check fails. Packages are signed with a throwaway in-memory key
(needs the `cryptography` package; skipped without it). Runs without root.
"""
import base64,hashlib,io,json,os,pathlib,shutil,socket,subprocess,sys,tarfile,tempfile,time,unittest
HERE=pathlib.Path(__file__).resolve().parent;ROOT=HERE.parent;UPDATE=ROOT/'packaging/nabiman-update'
sys.path.insert(0,str(HERE))
try:
 from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
 from cryptography.hazmat.primitives import serialization
except ImportError:Ed25519PrivateKey=None
SERVER='''#!/usr/bin/env python3
import http.server,os,sys
VERSION=%r
if VERSION.endswith('bad'):sys.exit(3)
class H(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  code,body=(200,b'<html>'+VERSION.encode()+b'</html>') if self.path=='/' else (401,b'{}') if self.path.startswith('/api/') else (404,b'')
  self.send_response(code);self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
 def log_message(self,*a):pass
http.server.HTTPServer(('127.0.0.1',int(os.environ['PORT'])),H).serve_forever()
'''
def package(path,version,arch):
 files={'nabiman-server':(SERVER%version).encode(),'nabiman-privileged-helper':b'#!/bin/sh\n','nabiman-recovery':b'#!/bin/sh\n','management-recovery.py':b'#\n',
  'verify-release.py':(HERE/'verify-release.py').read_bytes(),'static/index.html':b'<html>'+version.encode()+b'</html>','static/release-inventory.json':b'{}','static/release-bom.cdx.json':b'{}',
  'LICENSE':b'x','NOTICE':b'x','THIRD_PARTY_LICENSES.md':b'x','VERSION':(version+'\n').encode(),'nabiman.service':b'[Service]\n','packaging/nabiman-update':UPDATE.read_bytes()}
 mode=lambda n:0o755 if n in ('nabiman-server','nabiman-privileged-helper','nabiman-recovery','management-recovery.py','verify-release.py','packaging/nabiman-update') else 0o644
 manifest={'schema_version':1,'kind':'unsigned-package-file-manifest','files':{'nabiman/'+n:{'sha256':hashlib.sha256(b).hexdigest(),'size':len(b),'mode':mode(n)} for n,b in files.items()}}
 with tarfile.open(path,'w:gz') as t:
  for n,b in list(files.items())+[('release-manifest.json',json.dumps(manifest).encode())]:
   m=tarfile.TarInfo('nabiman/'+n);m.size=len(b);m.mode=mode(n) if n!='release-manifest.json' else 0o644;t.addfile(m,io.BytesIO(b))
@unittest.skipIf(Ed25519PrivateKey is None,'cryptography is not installed: cannot sign fixtures')
class Update(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.dir=pathlib.Path(self.tmp.name);self.prefix=self.dir/'root'
  self.arch={'x86_64':'x86_64','aarch64':'aarch64','arm64':'aarch64'}[os.uname().machine];self.asset='nabiman-linux-'+self.arch+'.tar.gz'
  self.key=Ed25519PrivateKey.generate();self.key_id=os.urandom(8)
  raw=self.key.public_key().public_bytes(serialization.Encoding.Raw,serialization.PublicFormat.Raw)
  (self.prefix/'etc/nabiman').mkdir(parents=True);(self.prefix/'etc/nabiman/release.pub').write_text('untrusted comment: test\n'+base64.b64encode(b'Ed'+self.key_id+raw).decode()+'\n')
  with socket.socket() as s:s.bind(('127.0.0.1',0));self.port=s.getsockname()[1]
  restart=self.dir/'restart.sh';self.pidfile=self.dir/'pid'
  restart.write_text(f'''pid=$(cat {self.pidfile} 2>/dev/null) && kill $pid 2>/dev/null; sleep 0.3
PORT={self.port} setsid {self.prefix}/usr/local/bin/nabiman-server >/dev/null 2>&1 < /dev/null & echo $! > {self.pidfile}
''')
  self.env=dict(os.environ,NABIMAN_PREFIX=str(self.prefix),NABIMAN_RESTART_CMD='bash '+str(restart))
 def tearDown(self):
  try:os.kill(int(self.pidfile.read_text()),15)
  except (OSError,ValueError):pass
  self.tmp.cleanup()
 def build(self,version,sign=True,comment_file=None):
  d=self.dir/('pkg-'+version);d.mkdir(exist_ok=True);archive=d/self.asset;package(archive,version,self.arch)
  if sign:
   message=hashlib.blake2b(archive.read_bytes(),digest_size=64).digest();sig=self.key.sign(message)
   comment='nabiman version:'+version+' file:'+(comment_file or self.asset);glob=self.key.sign(sig+comment.encode())
   (d/(self.asset+'.minisig')).write_text('untrusted comment: t\n'+base64.b64encode(b'ED'+self.key_id+sig).decode()+'\ntrusted comment: '+comment+'\n'+base64.b64encode(glob).decode()+'\n')
  return archive
 def run_update(self,*args):
  return subprocess.run(['bash',str(UPDATE),'--health-url',f'http://127.0.0.1:{self.port}','--health-timeout','8',*args],env=self.env,capture_output=True,text=True)
 def install(self,version):
  # As install.sh does: verify and extract, then place.
  archive=self.build(version);out=self.dir/('x-'+version)
  subprocess.run([sys.executable,str(HERE/'verify-release.py'),str(archive),'--public-key-file',str(self.prefix/'etc/nabiman/release.pub'),'--require-signature','--extract-to',str(out)],check=True,capture_output=True)
  r=self.run_update('--from-verified-dir',str(out/'nabiman'));self.assertEqual(r.returncode,0,r.stderr)
 def current(self):return os.readlink(self.prefix/'usr/local/lib/nabiman/current')
 def answers(self):
  import urllib.request
  with urllib.request.urlopen(f'http://127.0.0.1:{self.port}/',timeout=3) as r:return r.read().decode()
 def test_update_rollback_and_refusals(self):
  self.install('v1.0.0');self.assertEqual(self.current(),'v1.0.0')
  self.assertEqual(os.readlink(self.prefix/'usr/local/bin/nabiman-server'),str(self.prefix/'usr/local/lib/nabiman/current/nabiman-server'))
  self.assertEqual(os.readlink(self.prefix/'usr/local/share/nabiman/static'),str(self.prefix/'usr/local/lib/nabiman/current/static'))
  r=self.run_update('--archive',str(self.build('v1.1.0')));self.assertEqual(r.returncode,0,r.stdout+r.stderr)
  self.assertEqual(self.current(),'v1.1.0');self.assertIn('v1.1.0',self.answers())
  # A version that does not come up: back to v1.1.0 by itself, previous still v1.0.0.
  r=self.run_update('--archive',str(self.build('v1.2.0-bad')),'--allow-downgrade')
  self.assertNotEqual(r.returncode,0);self.assertIn('rolled back to v1.1.0',r.stderr)
  self.assertEqual(self.current(),'v1.1.0');self.assertIn('v1.1.0',self.answers())
  self.assertEqual(os.readlink(self.prefix/'usr/local/lib/nabiman/previous'),'v1.0.0')
  # Unsigned, signed for another file, older without --allow-downgrade: refused, nothing changes.
  for archive,why in [(self.build('v1.3.0',sign=False),'unsigned'),(self.build('v1.4.0',comment_file='other.tar.gz'),'other file'),(self.build('v0.9.0'),'downgrade')]:
   with self.subTest(why):
    r=self.run_update('--archive',str(archive));self.assertNotEqual(r.returncode,0,why);self.assertEqual(self.current(),'v1.1.0')
  r=self.run_update('--rollback');self.assertEqual(r.returncode,0,r.stderr);self.assertEqual(self.current(),'v1.0.0');self.assertIn('v1.0.0',self.answers())
  listing=self.run_update('--list').stdout;self.assertIn('v1.0.0 (current)',listing);self.assertIn('v1.1.0 (previous)',listing)
 def test_keep_and_legacy_adoption(self):
  bin_dir=self.prefix/'usr/local/bin';bin_dir.mkdir(parents=True);(bin_dir/'nabiman-server').write_text((SERVER%'legacy'));(bin_dir/'nabiman-server').chmod(0o755)
  (self.prefix/'usr/local/share/nabiman/static').mkdir(parents=True);(self.prefix/'usr/local/share/nabiman/static/index.html').write_text('<html>legacy</html>')
  self.install('v2.0.0');lib=self.prefix/'usr/local/lib/nabiman'
  legacy=[p.name for p in lib.iterdir() if p.name.startswith('legacy-')];self.assertEqual(len(legacy),1)
  self.assertEqual(os.readlink(lib/'previous'),legacy[0]);self.assertTrue((lib/legacy[0]/'static/index.html').is_file())
  for v in ['v2.1.0','v2.2.0','v2.3.0']:
   r=self.run_update('--archive',str(self.build(v)),'--keep','3');self.assertEqual(r.returncode,0,r.stderr);time.sleep(0.05)
  kept=sorted(p.name for p in lib.iterdir() if p.is_dir() and not p.is_symlink() and not p.name.startswith('.'))
  self.assertEqual(kept,['v2.1.0','v2.2.0','v2.3.0'])
if __name__=='__main__':unittest.main()
