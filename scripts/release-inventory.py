#!/usr/bin/env python3
"""Packaging-time lockfile inventory and artifact receipt, not a signed build attestation."""
import argparse,base64,datetime,hashlib,json,pathlib,subprocess,urllib.parse,uuid
EXCLUDED={'release-bom.cdx.json','release-inventory.json'}
def sha(path):
 with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest() if hasattr(hashlib,'file_digest') else hashlib.sha256(f.read()).hexdigest()
def write(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def files(root):
 result={}
 for p in sorted(root.rglob('*')):
  if p.is_symlink():raise ValueError('Release inputs must not contain symlinks')
  if p.is_file() and p.relative_to(root).as_posix() not in EXCLUDED:result[p.relative_to(root).as_posix()]={'sha256':sha(p),'size':p.stat().st_size}
 return result
def ref(kind,key):return 'urn:nabiman:'+kind+':'+hashlib.sha256(key.encode()).hexdigest()
def purl(kind,name,version):return 'pkg:'+kind+'/'+urllib.parse.quote(name,safe='/')+'@'+urllib.parse.quote(version,safe='')
def inventory(source):
 try:import tomllib
 except ImportError:import tomli as tomllib
 cargo=tomllib.loads((source/'backend/Cargo.lock').read_text())['package']
 npm=json.loads((source/'frontend/package-lock.json').read_text());assert npm['lockfileVersion']==3
 components=[];edges=[];cargo_refs={};by_name={}
 for row in cargo:
  key=(row['name'],row['version'],row.get('source',''));ident=ref('cargo',json.dumps(key));cargo_refs[key]=ident;by_name.setdefault(row['name'],[]).append((key,ident))
  c={'type':'application' if not row.get('source') else 'library','bom-ref':ident,'name':row['name'],'version':row['version'],'purl':purl('cargo',row['name'],row['version'])}
  if 'checksum' in row:c['hashes']=[{'alg':'SHA-256','content':row['checksum']}]
  c['properties']=[{'name':'nabiman:inventory','value':'Cargo.lock; may include dev, build and non-target dependencies; license not declared in lockfile'}]
  components.append(c)
 for row in cargo:
  targets=[]
  for dependency in row.get('dependencies',[]):
   parts=dependency.split(' ',2);matches=[(key,ident) for key,ident in by_name.get(parts[0],[]) if (len(parts)<2 or parts[1]==key[1]) and (len(parts)<3 or parts[2].strip('()')==key[2])]
   if len(matches)!=1:raise ValueError('Ambiguous locked Cargo dependency')
   targets.append(matches[0][1])
  edges.append({'ref':cargo_refs[(row['name'],row['version'],row.get('source',''))],'dependsOn':sorted(set(targets))})
 packages=npm['packages'];npm_refs={k:ref('npm',k) for k in packages};unresolved=0
 for path,row in packages.items():
  if row.get('link'):raise ValueError('Linked npm packages require a reviewed inventory implementation')
  name=row.get('name') or (path.rsplit('node_modules/',1)[-1] if path else npm['name']);version=row.get('version')
  if not version:raise ValueError('Unversioned npm package')
  c={'type':'library' if path else 'application','bom-ref':npm_refs[path],'name':name,'version':version,'purl':purl('npm',name,version),
     'properties':[{'name':'nabiman:lock-path','value':path or '.'},{'name':'nabiman:dev','value':str(row.get('dev',False)).lower()},{'name':'nabiman:optional','value':str(row.get('optional',False)).lower()}]}
  if isinstance(row.get('license'),str):c['licenses']=[{'license':{'name':row['license']}}]
  integrity=row.get('integrity','').split()
  for item in integrity:
   algorithm,sep,digest=item.partition('-')
   if sep and algorithm in ['sha256','sha384','sha512']:
    decoded=base64.b64decode(digest,validate=True)
    if len(decoded)!=int(algorithm[3:])//8:raise ValueError('Invalid npm integrity length')
    c.setdefault('hashes',[]).append({'alg':'SHA-'+algorithm[3:],'content':decoded.hex()})
  components.append(c);targets=[]
  names=set(row.get('dependencies',{}))|set(row.get('optionalDependencies',{}))|set(row.get('devDependencies',{}))
  # Peer resolution is environment-dependent; report it as incomplete rather than guessing.
  if row.get('peerDependencies'):c['properties'].append({'name':'nabiman:peer-graph','value':'not resolved by this lock inventory'})
  for name in names:
   current=path
   while True:
    candidate=(current+'/' if current else '')+'node_modules/'+name
    if candidate in npm_refs:targets.append(npm_refs[candidate]);break
    if not current:unresolved+=1;break
    current=current.rsplit('/node_modules/',1)[0] if '/node_modules/' in current else ''
  edges.append({'ref':npm_refs[path],'dependsOn':sorted(set(targets))})
 root='urn:nabiman:release';roots=[ident for key,ident in cargo_refs.items() if not key[2]]+[npm_refs['']]
 return {'bomFormat':'CycloneDX','specVersion':'1.6','serialNumber':'urn:uuid:'+str(uuid.uuid4()),'version':1,
  'metadata':{'timestamp':datetime.datetime.now(datetime.timezone.utc).isoformat(),'lifecycles':[{'phase':'design'}],
   'component':{'type':'application','bom-ref':root,'name':'NabiMan','version':npm['version']},
   'properties':[{'name':'nabiman:scope','value':'Declared lockfile inventory, including development/optional/platform packages; not measured binary or browser-bundle composition. System libraries and resolved peer graph are incomplete.'},{'name':'nabiman:unresolved-npm-edges','value':str(unresolved)}]},
  'components':components,'dependencies':[{'ref':root,'dependsOn':roots}]+edges,'compositions':[{'aggregate':'incomplete','assemblies':[root]}]}
def generate(source,binary,static):
 bom=inventory(source);write(static/'release-bom.cdx.json',bom)
 artifacts={name:{'sha256':sha(binary.parent/name),'size':(binary.parent/name).stat().st_size} for name in ['nabiman-server','nabiman-privileged-helper','nabiman-recovery']}
 receipt={'schema_version':1,'kind':'unsigned-packaging-inventory','created_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
  'artifacts':artifacts,'inputs':{name:sha(source/name) for name in ['backend/Cargo.lock','frontend/package-lock.json','scripts/management-recovery.py']},
  'static':files(static),'sbom_sha256':sha(static/'release-bom.cdx.json'),'components':len(bom['components']),
  'limitations':['Lockfile inventory is not measured artifact composition','No source-to-binary, reproducible-build or publisher-signature proof','No complete license review, vulnerability scan or system-library inventory']}
 write(static/'release-inventory.json',receipt);verify(source,binary,static)
 print(json.dumps({'components':receipt['components'],'unsigned':True,'artifact_checks_passed':True}))
def verify(source,binary,static):
 receipt=json.loads((static/'release-inventory.json').read_text());assert receipt['schema_version']==1 and receipt['kind']=='unsigned-packaging-inventory'
 for name,entry in receipt['artifacts'].items():
  assert name in ['nabiman-server','nabiman-privileged-helper','nabiman-recovery'];p=binary.parent/name;assert sha(p)==entry['sha256'] and p.stat().st_size==entry['size']
 assert set(receipt['artifacts'])=={'nabiman-server','nabiman-privileged-helper','nabiman-recovery'}
 for name in ['backend/Cargo.lock','frontend/package-lock.json','scripts/management-recovery.py']:assert receipt['inputs'][name]==sha(source/name)
 for name,entry in receipt['static'].items():
  path=pathlib.PurePosixPath(name);assert not path.is_absolute() and '..' not in path.parts
  p=static/name;assert not p.is_symlink() and sha(p)==entry['sha256'] and p.stat().st_size==entry['size']
 assert receipt['sbom_sha256']==sha(static/'release-bom.cdx.json')
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('action',choices=['generate','verify']);parser.add_argument('--source',type=pathlib.Path,required=True);parser.add_argument('--binary',type=pathlib.Path,required=True);parser.add_argument('--static',type=pathlib.Path,required=True);a=parser.parse_args()
 (generate if a.action=='generate' else verify)(a.source.resolve(),a.binary.resolve(),a.static.resolve())
