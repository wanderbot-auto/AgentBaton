import type { Language } from './i18n';
import { experienceLimits, type ExperienceKind } from './task-experience-proposal';

const { summary, content } = experienceLimits;

export const taskExperienceMessages = {
  en: {
    title: 'Experience candidate',
    help: 'Summarize reusable experience from this accepted task. The draft is temporary and stays in this view for review.',
    open: 'Create experience candidate',
    kind: 'Candidate type',
    kinds: { skill: 'Skill', template: 'Template', rule: 'Rule' },
    starters: {
      skill: 'When to use\n\nSteps to repeat\n\nHow to verify the result',
      template: 'Purpose\n\nRequired inputs\n\nExpected output\n\nAcceptance checks',
      rule: 'When this applies\n\nExpected behavior\n\nExceptions',
    },
    summary: 'Summary',
    content: 'Reusable content',
    invalid: {
      summary: `Enter a summary of 1–${summary} characters.`,
      content: `Enter reusable content of 1–${content} characters.`,
    },
    review: 'Preview candidate',
    preview: 'Candidate preview',
    edit: 'Edit candidate',
    cancel: 'Discard candidate',
    project: 'Project',
    task: 'Task ID',
    revision: 'Task revision',
    specRevision: 'Specification revision',
    acceptance: 'Acceptance reference',
    sequence: 'Acceptance event sequence',
  },
  zh: {
    title: '经验候选',
    help: '从已验收任务中总结可复用的经验。草稿仅在当前视图中临时保留，供审阅。',
    open: '创建经验候选',
    kind: '候选类型',
    kinds: { skill: 'Skill', template: '模板', rule: '规则' },
    starters: {
      skill: '适用场景\n\n可复用步骤\n\n结果验证方法',
      template: '用途\n\n必填输入\n\n预期输出\n\n验收检查',
      rule: '适用条件\n\n预期行为\n\n例外情况',
    },
    summary: '摘要',
    content: '可复用内容',
    invalid: {
      summary: `请输入 1 至 ${summary} 个字符的摘要。`,
      content: `请输入 1 至 ${content} 个字符的可复用内容。`,
    },
    review: '预览候选',
    preview: '候选预览',
    edit: '编辑候选',
    cancel: '放弃候选',
    project: '项目',
    task: '任务 ID',
    revision: '任务修订',
    specRevision: '规格修订',
    acceptance: '验收引用',
    sequence: '验收事件序号',
  },
} satisfies Record<Language, ExperienceMessages>;

type ExperienceMessages = Record<
  'title' | 'help' | 'open' | 'kind' | 'summary' | 'content' | 'review' | 'preview' |
  'edit' | 'cancel' | 'project' | 'task' | 'revision' | 'specRevision' | 'acceptance' | 'sequence',
  string
> & {
  kinds: Record<ExperienceKind, string>;
  starters: Record<ExperienceKind, string>;
  invalid: Record<keyof typeof experienceLimits, string>;
};
