import type { Task } from './api';
import type { Language } from './i18n';
import { taskActionsMessages } from './task-actions-i18n';
export type TaskAction = 'workspace' | 'plan' | 'review';
type Props = { task:Task; language:Language; busy:boolean; sendBusy:boolean; connected:boolean; canSend:boolean; recipient?:string; multipleSessions:boolean; uncertain:boolean; receiptReviewHint?:string; onNavigate:(task:Task,action:TaskAction)=>void; onSend:(task:Task)=>void };
export function TaskActions({task,language,busy,sendBusy,connected,canSend,recipient,multipleSessions,uncertain,receiptReviewHint,onNavigate,onSend}:Props) {
 const t=taskActionsMessages[language];
 return <section className="thread-note" aria-label={t.title}><h3>{t.title}</h3><p>{t.hint}</p><div className="dialog-actions">{(['workspace','plan','review'] as const).map(action=><button type="button" key={action} disabled={busy} onClick={()=>onNavigate(task,action)}>{t[action]}</button>)}</div><details><summary>{t.preview}</summary><blockquote>{task.goal}</blockquote></details><p>{recipient?`${t.recipient}: ${recipient}`:multipleSessions?t.multiple:t.unavailable}</p><button type="button" disabled={busy||sendBusy||!connected||!canSend||uncertain} onClick={()=>onSend(task)}>{t.send}</button><p className="hint">{receiptReviewHint ?? (uncertain?t.uncertain:!canSend?t.unavailable:t.sendHint)}</p>{!connected&&<p>{t.offline}</p>}</section>;
}
