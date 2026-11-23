import { Button, Form, Select, Textarea, Title } from './ui';
import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import type { Project, Task } from './api';
import type { Language } from './i18n';
import { taskExperienceMessages } from './task-experience-proposal-i18n';
import {
  createExperienceCandidate, experienceKinds, experienceSource, initialExperienceDraft,
  type ExperienceCandidate, type ExperienceDraft, type ExperienceKind, type ExperienceSource,
} from './task-experience-proposal';

type Props = { task: Task; project?: Project; selectedProjectId: string; language: Language };
type View = { phase: 'closed' } | { phase: 'edit'; draft: ExperienceDraft } |
  { phase: 'preview'; candidate: ExperienceCandidate };

export function TaskExperienceProposal({ task, project, selectedProjectId, language }: Props) {
  const source = experienceSource(task, project, selectedProjectId);
  if (!source) return null;
  // Keep temporary text scoped to the rendered, accepted revision and project.
  return <ExperienceEditor key={JSON.stringify([source, task.goal, selectedProjectId])}
    source={source} goal={task.goal} language={language} />;
}

function ExperienceEditor({ source, goal, language }: { source: ExperienceSource; goal: string; language: Language }) {
  const t = taskExperienceMessages[language];
  const id = useId();
  const [view, setView] = useState<View>({ phase: 'closed' });
  const [invalid, setInvalid] = useState<'summary' | 'content'>();
  const entry = useRef<HTMLButtonElement>(null);
  const kindSelect = useRef<HTMLSelectElement>(null);
  const previewHeading = useRef<HTMLHeadingElement>(null);
  const restoreEntryFocus = useRef(false);

  useEffect(() => {
    if (view.phase === 'edit') kindSelect.current?.focus();
    if (view.phase === 'preview') previewHeading.current?.focus();
    if (view.phase === 'closed' && restoreEntryFocus.current) {
      entry.current?.focus();
      restoreEntryFocus.current = false;
    }
  }, [view.phase]);

  function discard() {
    restoreEntryFocus.current = true;
    setInvalid(undefined);
    setView({ phase: 'closed' });
  }

  function update(draft: ExperienceDraft) {
    setInvalid(undefined);
    setView({ phase: 'edit', draft });
  }

  function preview(event: FormEvent<HTMLFormElement>, draft: ExperienceDraft, content: string) {
    event.preventDefault();
    const result = createExperienceCandidate(source, { ...draft, content });
    if (result.invalidField) {
      setInvalid(result.invalidField);
      const field = event.currentTarget.elements.namedItem(result.invalidField);
      if (field instanceof HTMLTextAreaElement) field.focus();
      return;
    }
    setView({ phase: 'preview', candidate: result.candidate });
  }

  const draft = view.phase === 'edit' ? view.draft : null;
  const content = draft ? draft.content ?? t.starters[draft.kind] : '';
  const candidate = view.phase === 'preview' ? view.candidate : null;
  return <section aria-labelledby={`${id}-title`}>
    <Title order={2} id={`${id}-title`}>{t.title}</Title>
    <p className="hint">{t.help}</p>
    {view.phase === 'closed' && <Button type="button" ref={entry}
      onClick={() => update(initialExperienceDraft(goal))}>{t.open}</Button>}
    {draft && <Form className="connection-form" onSubmit={event => preview(event, draft, content)}>
      <label htmlFor={`${id}-kind`}>{t.kind}</label>
      <Select id={`${id}-kind`} ref={kindSelect} value={draft.kind}
        onChange={event => update({ ...draft, kind: event.target.value as ExperienceKind })}>
        {experienceKinds.map(kind => <option key={kind} value={kind}>{t.kinds[kind]}</option>)}
      </Select>
      <label htmlFor={`${id}-summary`}>{t.summary}</label>
      <Textarea id={`${id}-summary`} name="summary" rows={2} value={draft.summary}
        aria-invalid={invalid === 'summary' || undefined}
        aria-describedby={invalid === 'summary' ? `${id}-error` : undefined}
        onChange={event => update({ ...draft, summary: event.target.value })} />
      <label htmlFor={`${id}-content`}>{t.content}</label>
      <Textarea id={`${id}-content`} name="content" rows={6} value={content}
        aria-invalid={invalid === 'content' || undefined}
        aria-describedby={invalid === 'content' ? `${id}-error` : undefined}
        onChange={event => update({ ...draft, content: event.target.value })} />
      {invalid && <p id={`${id}-error`} role="alert">{t.invalid[invalid]}</p>}
      <div className="dialog-actions" style={{ gridColumn: '1 / -1' }}>
        <Button type="button" onClick={discard}>{t.cancel}</Button>
        <Button type="submit" className="primary">{t.review}</Button>
      </div>
    </Form>}
    {candidate && <div style={{ overflowWrap: 'anywhere' }}>
      <Title order={3} ref={previewHeading} tabIndex={-1}>{t.preview}</Title>
      <dl>
        <dt>{t.kind}</dt><dd>{t.kinds[candidate.kind]}</dd>
        <dt>{t.project}</dt><dd>{candidate.source.projectName} ({candidate.source.projectId})</dd>
        <dt>{t.task}</dt><dd>{candidate.source.taskId}</dd>
        <dt>{t.revision}</dt><dd>{candidate.source.taskRevision}</dd>
        <dt>{t.specRevision}</dt><dd>{candidate.source.specRevision}</dd>
        <dt>{t.acceptance}</dt><dd>{candidate.source.acceptanceRef}</dd>
        <dt>{t.sequence}</dt><dd>{candidate.source.acceptanceSequence}</dd>
      </dl>
      <Title order={4}>{t.summary}</Title>
      <p style={{ whiteSpace: 'pre-wrap' }}>{candidate.summary}</p>
      <Title order={4}>{t.content}</Title>
      <p style={{ whiteSpace: 'pre-wrap' }}>{candidate.content}</p>
      <div className="dialog-actions">
        <Button type="button" onClick={discard}>{t.cancel}</Button>
        <Button type="button" onClick={() => update({
          kind: candidate.kind, summary: candidate.summary, content: candidate.content,
        })}>{t.edit}</Button>
      </div>
    </div>}
  </section>;
}
