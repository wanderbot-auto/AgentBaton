# Added only to the independent bounded-rework generated ACP test program.
# The fixed grep validator reads the selected artifact, not this program's state.
def rework_plan(ctx, revision):
 import copy
 p=child_plan(ctx,revision);work=p['nodes'][0]
 work['goal']='Produce the first deliberately invalid exact report' if revision==1 else 'Produce the repaired exact report'
 work['budget']['children']=0
 work['reservation']['runtime_ms']=4000
 owners=ctx['required_acceptance_rules'];assert len(owners)==1
 checks=[rule for rule in ctx['contracts']['output:report']if rule not in owners];assert len(checks)==1 and checks[0].startswith('command:')
 candidate={'goal_task_id':ctx['goal']['task_id'],'artifact_id':'report','contract_id':'output:report','source':{'kind':'node','node_id':work['id'],'candidate_for_validation':True}}
 validator=copy.deepcopy(work)
 validator.update({'id':'validate-report','kind':'validate','goal':'Run the same approved exact grep command on this owned candidate','dependencies':[work['id']],'executor':{'kind':'validator','rule_id':checks[0]},'outputs':[],'required_inputs':[candidate],'required_validators':checks,'side_effect':'advisory_read_only','write_set':[],'reservation':{'runtime_ms':2000,'output_bytes':1024,'tokens':None,'cost_microunits':None},'idempotency_key':'fixed-validator'})
 owner=copy.deepcopy(validator)
 owner.update({'id':'owner-report','kind':'owner_approval','goal':'Review the repaired complete report after deterministic validation','dependencies':[work['id'],validator['id']],'executor':{'kind':'owner','rule_id':owners[0]},'required_validators':owners,'reservation':{'runtime_ms':1000,'output_bytes':1024,'tokens':None,'cost_microunits':None},'idempotency_key':'fixed-owner'})
 p['nodes']=[work,validator,owner]
 return p

def rework_save(ctx, revision):
 p=rework_plan(ctx,revision);raw=json.dumps(p,indent=2)+'\n'
 args={'task_id':ctx['goal']['task_id'],'plan_json':raw,'expected_plan_revision':revision-1,'idempotency_key':'rework-save-'+str(revision)}
 saved=tool('submit_child_plan',args)
 assert saved['plan_json']==raw and saved['reference']['raw_sha256']==hashlib.sha256(raw.encode()).hexdigest()
 assert saved['reference']['revision']==revision and saved['planning_only']is True and saved['dispatched']is False
 assert tool('submit_child_plan',args)==saved
 return saved
