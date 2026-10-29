# Inserted only in the new planning mode of the real generated ACP fixture.
def child_plan(ctx, revision):
 import copy
 binding=copy.deepcopy(ctx['execution_bindings'][0]);outputs=[{'artifact_id':key,'contract_id':contract}for key,contract in ctx['outputs'].items()]
 required=sorted(set(rule for output in outputs for rule in ctx['contracts'][output['contract_id']]))
 inputs=[{'goal_task_id':ctx['goal']['task_id'],'artifact_id':key,'contract_id':value['contract_id'],'source':{'kind':'approved_external','sha256':value['sha256']}}for key,value in ctx['inputs'].items()]
 writes=[{'kind':'workspace','path':path}for path in ctx['edit_context']['writable_prefixes']]
 return {'schema_version':1,'plan_id':'plan-'+ctx['goal']['task_id'],'goal':copy.deepcopy(ctx['goal']),'scope_revision':ctx['scope_revision'],'revision':revision,'expected_revision':revision-1,'acceptance_rules':ctx['required_acceptance_rules'],'nodes':[{'id':'fixed-work','parent_id':None,'kind':'work','goal':'Plan only the exact approved outputs','dependencies':[],'workspace_id':binding['workspace_id'],'executor':{'kind':'connection','binding':binding,'program_sha256':ctx['program_sha256'][binding['connection_id']]},'recipients':[],'required_inputs':inputs,'outputs':outputs,'required_validators':required,'side_effect':'workspace_write'if writes else'advisory_read_only','write_set':writes,'budget':copy.deepcopy(ctx['budget']),'reservation':{'runtime_ms':1000,'output_bytes':1024,'tokens':None,'cost_microunits':None},'retry':{'max_attempts':1,'max_reworks':0,'retry_classes':[]},'idempotency_key':'fixed-plan-work','external_action_id':None}]}
def submit_plan(ctx, revision, key):
 p=child_plan(ctx,revision)
 args={'task_id':ctx['goal']['task_id'],'plan_json':json.dumps(p,indent=2)+'\n','expected_plan_revision':revision-1,'idempotency_key':key}
 value=tool('submit_child_plan',args)
 assert value['planning_only']is True and value['dispatched']is False and value['task_accepted']is False
 assert value['plan_json']==args['plan_json']and value['reference']['revision']==revision
 assert value['reference']['raw_sha256']==hashlib.sha256(args['plan_json'].encode()).hexdigest()
 assert tool('submit_child_plan',args)==value
 return value,args
