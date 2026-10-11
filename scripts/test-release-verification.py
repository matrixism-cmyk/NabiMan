#!/usr/bin/env python3
import hashlib,importlib.util,io,json,pathlib,tarfile,tempfile,unittest
spec=importlib.util.spec_from_file_location('verify',pathlib.Path(__file__).with_name('verify-release.py'));v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
REQUIRED=['nabiman-server','nabiman-privileged-helper','nabiman-recovery','management-recovery.py','verify-release.py','static/index.html','static/release-inventory.json','static/release-bom.cdx.json']
class Verification(unittest.TestCase):
 def archive(self,root,mutation=None):
  contents={'nabiman/'+name:b'fixture' for name in REQUIRED};entries={k:{'sha256':hashlib.sha256(b).hexdigest(),'size':len(b),'mode':0o755} for k,b in contents.items()}
  manifest={'schema_version':1,'kind':'unsigned-package-file-manifest','files':entries}
  rows=[]
  for name,data in contents.items():
   m=tarfile.TarInfo(name);m.size=len(data);m.mode=0o755;rows.append([m,data])
  data=json.dumps(manifest).encode();m=tarfile.TarInfo(v.MANIFEST);m.size=len(data);m.mode=0o644;rows.append([m,data])
  if mutation:mutation(rows)
  path=root/'package.tar.gz'
  with tarfile.open(path,'w:gz') as f:
   for m,data in rows:f.addfile(m,io.BytesIO(data) if m.isfile() else None)
  return path
 def test_new_directory_and_no_overwrite(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=pathlib.Path(tmp);archive=self.archive(root);target=root/'new';proof=v.verify(archive,target);self.assertFalse(proof['publisher_authenticated']);self.assertEqual((target/'nabiman/nabiman-server').read_bytes(),b'fixture')
   with self.assertRaises(ValueError):v.verify(archive,target)
 def test_corruption_missing_and_extra_files(self):
  for action in ['corrupt','missing','extra','mode','duplicate']:
   with self.subTest(action=action),tempfile.TemporaryDirectory() as tmp:
    def mutate(rows):
     if action=='corrupt':rows[0][1]=b'changed'
     elif action=='missing':rows.pop(0)
     elif action=='extra':m=tarfile.TarInfo('nabiman/extra');m.size=1;rows.append([m,b'x'])
     elif action=='mode':rows[0][0].mode=0o644
     else:rows.append(rows[0])
    root=pathlib.Path(tmp);archive=self.archive(root,mutate)
    with self.assertRaises(ValueError):v.verify(archive,root/'target')
    self.assertFalse((root/'target').exists())
 def test_unsafe_archive_types_and_paths(self):
  for name,kind in [('../escape',tarfile.REGTYPE),('/escape',tarfile.REGTYPE),('nabiman/../escape',tarfile.REGTYPE),('nabiman/link',tarfile.SYMTYPE),('nabiman/hard',tarfile.LNKTYPE),('nabiman/device',tarfile.CHRTYPE),('nabiman/fifo',tarfile.FIFOTYPE)]:
   with self.subTest(name=name),tempfile.TemporaryDirectory() as tmp:
    def mutate(rows):m=tarfile.TarInfo(name);m.type=kind;m.linkname='/outside';m.size=0;rows.append([m,b''])
    root=pathlib.Path(tmp)
    with self.assertRaises(ValueError):v.verify(self.archive(root,mutate),root/'target')
    self.assertFalse((root/'target').exists())
 def test_expansion_limit_and_parent_collisions(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=pathlib.Path(tmp);archive=self.archive(root);old=v.MAX_BYTES;v.MAX_BYTES=1
   try:
    with self.assertRaises(ValueError):v.verify(archive)
   finally:v.MAX_BYTES=old
  with tempfile.TemporaryDirectory() as tmp:
   def mutate(rows):
    metadata=json.loads(rows[-1][1]);metadata['files']['nabiman/static']={'sha256':hashlib.sha256(b'').hexdigest(),'size':0,'mode':0o644}
    data=json.dumps(metadata).encode();rows[-1][0].size=len(data);rows[-1][1]=data
    m=tarfile.TarInfo('nabiman/static');m.mode=0o644;m.size=0;rows.append([m,b''])
   with self.assertRaises(ValueError):v.verify(self.archive(pathlib.Path(tmp),mutate))
if __name__=='__main__':unittest.main()
