"""Real crontab round trips in the caller's private chroot, without a cron daemon."""
import json,pathlib,subprocess

def verify(root,call,token,base,repo,chromium):
 assert root.name.startswith('nb-os-account-root-') and root.parent==pathlib.Path('/tmp')
 command=['/usr/sbin/chroot',str(root),'/usr/bin/crontab','-u','root']
 def install(text):subprocess.run(command+['-'],input=text,text=True,capture_output=True,check=True,timeout=5)
 def read():return subprocess.run(command+['-l'],text=True,capture_output=True,check=True,timeout=5).stdout
 seed='# retained context\nSHELL=/bin/sh\n@reboot /bin/true\nMAILTO=""\n*\t* * * * /bin/echo fixture\n# last comment\n'
 install(seed);assert read()==seed
 rows=call('/api/cron',token=token)[1]['data'];assert [r['id'] for r in rows]==[3,5]
 old=rows[1]
 assert call('/api/cron/delete',{'user':'root','id':5},token)[0]==400
 assert read()==seed
 snapshot=call('/api/cron/snapshot?user=root',token=token)[1]['data'];assert snapshot['content']==seed and snapshot['can_write']
 # External edits invalidate an already reviewed selection without losing the new entry.
 changed=seed+'# edited externally\n';install(changed)
 assert call('/api/cron/delete',{'user':'root','id':5,'expected_revision':old['revision']},token)[0]==409
 assert read()==changed
 # Insertion before the selected physical line must not redirect stale deletion.
 inserted=seed.replace('*\t* * * * /bin/echo fixture','0 2 * * * /bin/echo external\n*\t* * * * /bin/echo fixture')
 install(inserted)
 assert call('/api/cron/delete',{'user':'root','id':5,'expected_revision':old['revision']},token)[0]==409
 assert read()==inserted
 install(changed)
 revision=call('/api/cron/snapshot?user=root',token=token)[1]['data']['revision']
 assert call('/api/cron/delete',{'user':'root','id':5,'expected_revision':revision},token)[0]==200
 expected=changed.replace('*\t* * * * /bin/echo fixture\n','');assert read()==expected
 revision=call('/api/cron/snapshot?user=root',token=token)[1]['data']['revision']
 for schedule,body in [('* * * * *','/bin/true\n* * * * * injected'),('99 99 * * *','/bin/true')]:
  assert call('/api/cron/add',{'user':'root','expected_revision':revision,'schedule':schedule,'command':body},token)[0]==409
  assert read()==expected
 assert call('/api/cron/add',{'user':'root','expected_revision':revision,'schedule':'0 1 * * *','command':'/bin/true'},token)[0]==200
 expected+='0 1 * * * /bin/true\n';assert read()==expected
 # An unavailable executable cannot be mistaken for an empty crontab or enable overwriting.
 tool=root/'usr/bin/crontab';tool.rename(tool.with_name('crontab.off'))
 try:
  assert call('/api/cron',token=token)[0]==503
  assert call('/api/cron/add',{'user':'root','expected_revision':revision,'schedule':'0 2 * * *','command':'/bin/true'},token)[0]==409
 finally:tool.with_name('crontab.off').rename(tool)
 assert read()==expected
 # Edit in place, including a real @schedule, while preserving all other bytes.
 revision=call('/api/cron/snapshot?user=root',token=token)[1]['data']['revision']
 assert call('/api/cron/update',{'user':'root','id':3,'schedule':'@hourly','command':'/bin/true'},token)[0]==400
 for target,schedule,command_text in [(1,'@daily','/bin/true'),(3,'@unknown','/bin/true'),(3,'@daily','/bin/true\n/bin/false'),(3,'99 99 * * *','/bin/true')]:
  assert call('/api/cron/update',{'user':'root','id':target,'expected_revision':revision,'schedule':schedule,'command':command_text},token)[0]==409
  assert read()==expected
 assert call('/api/cron/update',{'user':'root','id':3,'expected_revision':revision,'schedule':'@hourly','command':'/bin/echo updated'},token)[0]==200
 assert read()==expected.replace('@reboot /bin/true\n','@hourly /bin/echo updated\n')
 assert call('/api/cron/update',{'user':'root','id':3,'expected_revision':revision,'schedule':'@reboot','command':'/bin/true'},token)[0]==409
 revision=call('/api/cron/snapshot?user=root',token=token)[1]['data']['revision']
 assert call('/api/cron/update',{'user':'root','id':3,'expected_revision':revision,'schedule':'@reboot','command':'/bin/true'},token)[0]==200
 assert read()==expected
 trailing='# preserved\n* * * * * printf hello\\ \n'
 install(trailing)
 try:
  row=next(r for r in call('/api/cron',token=token)[1]['data'] if r['user']=='root')
  assert row['command']=='printf hello\\ '
  assert call('/api/cron/update',{'user':'root','id':row['id'],'expected_revision':row['revision'],'schedule':'@hourly','command':row['command']},token)[0]==200
  assert read()==trailing.replace('* * * * *','@hourly')
 finally:install(expected)
 result=subprocess.run(['node',str(repo/'frontend/scripts/smoke-cron.js')],input=json.dumps({'base':base,'token':token,'chromiumPath':chromium,'root':str(root)}),capture_output=True,text=True,timeout=60,cwd=repo/'frontend')
 assert result.returncode==0,'Cron browser fixture failed; command-bearing browser logs withheld'
 assert read()==expected,'Browser create/delete changed unrelated crontab bytes'
 return {'passed':True,'real_crontab_roundtrip':True,'in_place_edit_and_special_schedule_verified':True,'trailing_command_bytes_preserved':True,'environment_comments_order_preserved':True,'stale_revision_rejected':True,'insert_before_selected_line_rejected':True,'invalid_schedule_and_line_injection_rejected':True,'read_failure_never_overwrites':True,'browser':json.loads(result.stdout),'scheduled_commands_executed':False}
