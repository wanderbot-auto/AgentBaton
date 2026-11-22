import type { Task } from './api';
import type { Language } from './i18n';
import { taskActionsMessages } from './task-actions-i18n';
export type TaskAction = 'workspace' | 'plan' | 'review';
type Props = { task:Task; language:Language; busy:boolean; sendBusy:boolean; connected:boolean; canSend:boolean; recipient?:string; multipleSessions:boolean; uncertain:boolean; receiptReviewHint?:string; onNavigate:(task:Task,action:TaskAction,opener:HTMLButtonElement)=>void; onSend:(task:Task)=>void };
export function TaskActions({task,language,busy,sendBusy,connected,canSend,recipient,multipleSessions,uncertain,receiptReviewHint,onNavigate,onSend}:Props) {
 const t=taskActionsMessages[language];
 const feedback = receiptReviewHint ?? (uncertain ? t.uncertain : multipleSessions ? t.multiple : undefined);
 return <section className="task-actions" aria-label={t.title}>
   <div className="task-action-buttons">{(['workspace','plan','review'] as const).map(action => (
     <button type="button" key={action} disabled={busy} onClick={event => onNavigate(task,action,event.currentTarget)}>{t[action]}</button>
   ))}</div>
   <details><summary>{t.preview}</summary><blockquote>{task.goal}</blockquote></details>
   {recipient && <p>{t.recipient}: {recipient}</p>}
   <button type="button" disabled={busy||sendBusy||!connected||!canSend||uncertain} onClick={() => onSend(task)}>{t.send}</button>
   {feedback && <p className="operation-note" role="status">{feedback}</p>}
 </section>;
}
