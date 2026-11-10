"""Third-party licence inventory and policy for release packages.

Used by scripts/release-inventory.py (`licenses` action). Collects the SPDX licence
expression of every Cargo package (from `cargo metadata --locked`, offline when the
registry cache has everything) and every npm package (package-lock.json, cross-checked
with node_modules/*/package.json when installed), decides which of them ship, checks them
against packaging/licenses.json and writes NOTICE, THIRD_PARTY_LICENSES.md and
licenses.json.

"Ships" is: Cargo packages reachable from the server crate through normal (not dev or
build) dependencies on the release target platform; npm packages not marked dev in the
lockfile. Everything else is listed for information and never fails the release.
"""
import fnmatch,hashlib,json,pathlib,platform,re,subprocess

LICENSE_FILE=re.compile(r'^(licen[cs]e|copying|unlicense|notice|copyright|authors)([-._].*)?$',re.I)
NOTICE_FILE=re.compile(r'^notice([-._].*)?$',re.I)
ALLOWED,REVIEW,REJECTED=0,1,2

def target_triple(arch=None):
    return {'x86_64':'x86_64-unknown-linux-gnu','aarch64':'aarch64-unknown-linux-gnu','arm64':'aarch64-unknown-linux-gnu'}.get(arch or platform.machine(),None)

# --- SPDX expressions -------------------------------------------------------
def tokens(expression):
    text=expression.replace('/',' OR ').replace('(',' ( ').replace(')',' ) ')
    return [t if t.upper() not in ('AND','OR','WITH') else t.upper() for t in text.split()]

class Policy:
    def __init__(self,data):
        self.data=data;self.allow=set(data['allow']);self.conditions=dict(data.get('allow_with_conditions',{}))
        self.exceptions=set(data.get('exceptions',[]));self.aliases=dict(data.get('aliases',{}));self.review=list(data.get('review',[]))
    def normalize(self,expression):
        """Aliases for whole non-SPDX strings, and Cargo's legacy "MIT/Apache-2.0" as OR."""
        text=' '.join(expression.split())
        if text in self.aliases:return self.aliases[text]
        return ' '.join(text.replace('/',' OR ').split())
    def verdict_of_id(self,ident):
        if ident in self.allow or ident in self.conditions:return ALLOWED
        if any(fnmatch.fnmatchcase(ident,p) for p in self.review):return REVIEW
        return REJECTED
    def evaluate(self,expression):
        """(verdict, ids that decide it). OR takes the best branch, AND the worst."""
        stream=tokens(expression);pos=0
        def primary():
            nonlocal pos
            if pos>=len(stream):raise ValueError('incomplete expression')
            tok=stream[pos];pos+=1
            if tok=='(':
                result=disjunction()
                if pos>=len(stream) or stream[pos]!=')':raise ValueError('unbalanced parenthesis')
                pos+=1;return result
            if tok in ('AND','OR','WITH',')'):raise ValueError('unexpected '+tok)
            ident=self.aliases.get(tok,tok)
            if ident.endswith('+'):ident=ident[:-1]  # "GPL-2.0+": same family
            result=(self.verdict_of_id(ident),{ident})
            if pos<len(stream) and stream[pos]=='WITH':
                pos+=1;exception=stream[pos] if pos<len(stream) else '';pos+=1
                # An exception only ever relaxes a licence; an unknown one is not trusted.
                if exception not in self.exceptions:result=(max(result[0],REVIEW),{ident+' WITH '+exception})
                else:result=(result[0],{ident+' WITH '+exception})
            return result
        def conjunction():
            nonlocal pos
            verdict,ids=primary()
            while pos<len(stream) and stream[pos]=='AND':
                pos+=1;v,i=primary();verdict=max(verdict,v);ids=ids|i
            return verdict,ids
        def disjunction():
            nonlocal pos
            best=conjunction()
            while pos<len(stream) and stream[pos]=='OR':
                pos+=1;other=conjunction()
                if other[0]<best[0]:best=other
            return best
        result=disjunction()
        if pos!=len(stream):raise ValueError('trailing tokens')
        return result

# --- collection --------------------------------------------------------------
def license_files(directory):
    if not directory or not directory.is_dir():return []
    return sorted(p for p in directory.iterdir() if p.is_file() and LICENSE_FILE.match(p.name) and p.stat().st_size<512*1024)

def cargo_metadata(source,triple):
    base=['cargo','metadata','--format-version','1','--locked','--manifest-path',str(source/'backend/Cargo.toml')]
    if triple:base+=['--filter-platform',triple]
    for extra in (['--offline'],[]):
        run=subprocess.run(base+extra,capture_output=True,text=True)
        if run.returncode==0:return json.loads(run.stdout)
    raise RuntimeError('cargo metadata failed: '+run.stderr.strip()[-500:])

def cargo_packages(source,triple,policy):
    meta=cargo_metadata(source,triple);by_id={p['id']:p for p in meta['packages']}
    nodes={n['id']:n for n in meta['resolve']['nodes']};members=set(meta['workspace_members'])
    shipped=set();queue=list(members)
    while queue:
        current=queue.pop()
        if current in shipped:continue
        shipped.add(current)
        for dep in nodes[current]['deps']:
            if any(k['kind'] is None for k in dep['dep_kinds']):queue.append(dep['pkg'])
    rows=[]
    for pid,p in by_id.items():
        if pid in members:continue
        clarified=policy.data.get('clarify',{}).get('cargo:'+p['name'])
        expression=clarified['license'] if clarified else (p.get('license') or 'NOASSERTION')
        rows.append({'ecosystem':'cargo','name':p['name'],'version':p['version'],'license':policy.normalize(expression),
                     'declared':p.get('license') or ('license-file: '+p['license_file'] if p.get('license_file') else None),
                     'clarified':bool(clarified),'shipped':pid in shipped,'directory':pathlib.Path(p['manifest_path']).parent,
                     'repository':p.get('repository')})
    return rows

def npm_license(value):
    if isinstance(value,str):return value
    if isinstance(value,dict):return value.get('type')
    if isinstance(value,list):return ' OR '.join(filter(None,(npm_license(v) for v in value))) or None
    return None

def npm_packages(source,node_modules,policy):
    lock=json.loads((source/'frontend/package-lock.json').read_text());rows=[]
    root=node_modules.parent if node_modules else None
    for path,row in lock['packages'].items():
        if not path:continue
        name=row.get('name') or path.rsplit('node_modules/',1)[-1]
        directory=(root/path) if root else None;installed=None
        if directory and (directory/'package.json').is_file():
            try:installed=npm_license(json.loads((directory/'package.json').read_text()).get('license') or json.loads((directory/'package.json').read_text()).get('licenses'))
            except (ValueError,OSError):installed=None
        clarified=policy.data.get('clarify',{}).get('npm:'+name)
        declared=npm_license(row.get('license')) or installed
        expression=clarified['license'] if clarified else (declared or 'NOASSERTION')
        rows.append({'ecosystem':'npm','name':name,'version':row.get('version'),'license':policy.normalize(expression),'declared':declared,
                     'clarified':bool(clarified),'shipped':not row.get('dev',False),'directory':directory if directory and directory.is_dir() else None,
                     'lock_vs_installed_mismatch':bool(installed and row.get('license') and installed!=row.get('license')),'repository':row.get('resolved')})
    return rows

def collect(source,policy_path,node_modules=None,arch=None):
    policy=Policy(json.loads(policy_path.read_text()))
    if node_modules is None and (source/'frontend/node_modules').is_dir():node_modules=source/'frontend/node_modules'
    rows=cargo_packages(source,target_triple(arch),policy)+npm_packages(source,node_modules,policy)
    reviewed=policy.data.get('reviewed',{})
    for row in rows:
        try:verdict,ids=policy.evaluate(row['license'])
        except ValueError as error:verdict,ids=REJECTED,{'unparsable: '+str(error)}
        key=row['ecosystem']+':'+row['name']+'@'+str(row['version'])
        row['decided_by']=sorted(ids);row['conditions']=sorted(i for i in ids if i in policy.conditions)
        row['status']={ALLOWED:'allowed',REVIEW:'review',REJECTED:'rejected'}[verdict]
        if verdict==REVIEW and key in reviewed:row['status']='reviewed';row['review_reason']=reviewed[key]
        row['files']=license_files(row['directory'])
    rows.sort(key=lambda r:(r['ecosystem'],r['name'].lower(),str(r['version'])))
    return policy,rows

def problems(rows):
    return [r for r in rows if r['shipped'] and r['status'] in ('review','rejected')]

# --- output --------------------------------------------------------------------
def write_outputs(policy,rows,out,product_notice):
    out.mkdir(parents=True,exist_ok=True)
    shipped=[r for r in rows if r['shipped']];texts={};notices={}
    for r in shipped:
        for f in r['files']:
            data=f.read_text(errors='replace').strip();digest=hashlib.sha256(data.encode()).hexdigest()
            bucket=notices if NOTICE_FILE.match(f.name) else texts
            bucket.setdefault(digest,{'text':data,'packages':[]})['packages'].append(r['name']+' '+str(r['version']))
    counts={}
    for r in shipped:counts[r['license']]=counts.get(r['license'],0)+1
    lines=['# Third-party software in NabiMan','',
           'NabiMan includes the open-source packages below (Rust crates linked into the server binaries and npm packages bundled into the web interface).',
           'Each is used under the licence shown; licence and copyright texts follow. Generated by scripts/release-inventory.py from Cargo.lock and frontend/package-lock.json.','',
           '## Licences','','| Licence (SPDX) | Packages |','|---|---|']
    lines+=['| '+k.replace('|','\\|')+' | '+str(v)+' |' for k,v in sorted(counts.items(),key=lambda kv:(-kv[1],kv[0]))]
    conditions=sorted({c for r in shipped for c in r['conditions']})
    if conditions:
        lines+=['','Licences with conditions:','']+['- **'+c+'**: '+policy.conditions[c] for c in conditions]
    lines+=['','## Packages','','| Ecosystem | Package | Version | Licence |','|---|---|---|---|']
    lines+=['| '+r['ecosystem']+' | '+r['name']+' | '+str(r['version'])+' | '+r['license'].replace('|','\\|')+(' (clarified)' if r['clarified'] else '')+' |' for r in shipped]
    missing=[r['name']+' '+str(r['version']) for r in shipped if not any(not NOTICE_FILE.match(f.name) for f in r['files'])]
    lines+=['','## Licence texts','']
    for i,(digest,entry) in enumerate(sorted(texts.items(),key=lambda kv:sorted(kv[1]['packages'])[0]),1):
        lines+=['### Text '+str(i),'','Used by: '+', '.join(sorted(set(entry['packages']))),'','```',entry['text'].replace('```',"'''"),'```','']
    if missing:
        lines+=['### Packages without a licence file in their distribution','',
                'These packages declare the licence above but ship no licence text; the standard SPDX text of that licence applies:','',', '.join(sorted(missing)),'']
    (out/'THIRD_PARTY_LICENSES.md').write_text('\n'.join(lines)+'\n')
    notice=[product_notice.strip(),'','This product includes third-party software; see THIRD_PARTY_LICENSES.md for each package and its licence.',
            'The NOTICE files of those packages follow, as their licences (e.g. Apache-2.0 section 4(d)) require.','']
    for digest,entry in sorted(notices.items(),key=lambda kv:sorted(kv[1]['packages'])[0]):
        notice+=['-'*72,'From: '+', '.join(sorted(set(entry['packages']))),'',entry['text'],'']
    (out/'NOTICE').write_text('\n'.join(notice)+'\n')
    summary={'schema_version':1,'policy':'packaging/licenses.json','packages':[{k:(v if not isinstance(v,pathlib.Path) else None) for k,v in r.items() if k not in ('directory','files')}|{'license_files':[f.name for f in r['files']]} for r in rows],
             'shipped_licenses':counts,'problems':[r['ecosystem']+':'+r['name']+'@'+str(r['version'])+' '+r['license']+' ('+r['status']+')' for r in problems(rows)]}
    (out/'licenses.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
    return summary
