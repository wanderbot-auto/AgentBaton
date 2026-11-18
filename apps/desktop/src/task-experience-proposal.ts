import { taskHasAcceptanceProjection, type Project, type Task } from './api';

export const experienceKinds = ['skill', 'template', 'rule'] as const;
export type ExperienceKind = (typeof experienceKinds)[number];

export const experienceLimits = { summary: 500, content: 4000 } as const;

export function experienceSource(task: Task, project: Project | undefined, selectedProjectId: string) {
  const { id, project_id, revision, spec_revision, acceptance_ref, acceptance_sequence } = task;
  if (!taskHasAcceptanceProjection(task) || !project || project.id !== project_id) return null;
  if (selectedProjectId && selectedProjectId !== project.id) return null;
  if (spec_revision == null || acceptance_ref == null || acceptance_sequence == null) return null;

  return {
    taskId: id,
    projectId: project.id,
    projectName: project.name,
    taskRevision: revision,
    specRevision: spec_revision,
    acceptanceRef: acceptance_ref,
    acceptanceSequence: acceptance_sequence,
  };
}

export type ExperienceSource = NonNullable<ReturnType<typeof experienceSource>>;

type ExperienceText = { kind: ExperienceKind; summary: string; content: string };
export type ExperienceDraft = Omit<ExperienceText, 'content'> & { content?: string };
export type ExperienceCandidate = ExperienceText & { source: ExperienceSource };
type ExperienceField = keyof typeof experienceLimits;

export function initialExperienceDraft(goal: string): ExperienceDraft {
  return {
    kind: 'skill',
    summary: Array.from(goal.trim()).slice(0, experienceLimits.summary).join(''),
  };
}

export function createExperienceCandidate(source: ExperienceSource, text: ExperienceText):
  { candidate: ExperienceCandidate; invalidField?: never } |
  { candidate?: never; invalidField: ExperienceField } {
  const summary = text.summary.trim();
  const content = text.content.trim();
  if (!summary || Array.from(summary).length > experienceLimits.summary) return { invalidField: 'summary' };
  if (!content || Array.from(content).length > experienceLimits.content) return { invalidField: 'content' };
  return { candidate: { source, kind: text.kind, summary, content } };
}
