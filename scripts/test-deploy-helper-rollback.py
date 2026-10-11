#!/usr/bin/env python3
"""Exercise snapshot/rollback file restoration with all paths and supervisor isolated."""
import importlib.util,json,pathlib,sqlite3,tempfile
spec=importlib.util.spec_from_file_location('d',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'))
d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
for present,legacy in [(True,False),(False,False),(True,True)]:
 with tempfile.TemporaryDirectory(prefix='nb-helper-rollback-') as temporary:
  root=pathlib.Path(temporary);d.ROOT=root;d.AUTH=root/'auth';d.DB=root/'data/mec.db'
  for name in ['bin','auth','static','data','deployment']:(root/name).mkdir()
  (d.AUTH/'fixture.json').write_text('{"original":true}')
  with sqlite3.connect(d.DB) as db:db.execute('CREATE TABLE fixture(id INTEGER)')
  for name in ['nabiman-server','start-nabiman.sh','nabiman-recovery','management-recovery.py']:(root/'bin'/name).write_text('original-'+name)
  helper=root/'bin/nabiman-privileged-helper'
  if present:helper.write_bytes(b'old helper');helper.chmod(0o751)
  (root/'static/index.html').write_text('old UI')
  calls=[];d.supervisor=lambda action:calls.append(action);d.tmux_names=lambda:['retained-fixture']
  deployment=root/'deployment';d.snapshot(deployment/'cutover.before')
  if legacy:
   (deployment/'cutover.before/helper-presence.json').unlink();(deployment/'cutover.before/nabiman-privileged-helper').unlink()
  helper.write_bytes(b'new helper');helper.chmod(0o755)
  (root/'bin/nabiman-server').write_text('new API');(d.AUTH/'fixture.json').write_text('{"original":false}')
  d.rollback(deployment)
  assert calls==['stop','start']
  assert (root/'bin/nabiman-server').read_text()=='original-nabiman-server'
  assert json.loads((d.AUTH/'fixture.json').read_text())=={'original':True}
  assert (root/'static/index.html').read_text()=='old UI'
  assert json.loads((deployment/'rollback.json').read_text())['helper_restored']==(not legacy)
  if legacy:assert helper.read_bytes()==b'new helper'
  elif present:assert helper.read_bytes()==b'old helper' and helper.stat().st_mode&0o777==0o751
  else:assert not helper.exists()
print('PASS: private snapshot/rollback restores helper bytes and mode, or removes a newly introduced helper; no production paths or supervisor calls used')
