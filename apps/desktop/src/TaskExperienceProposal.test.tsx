import { editField } from './test-picker';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { expect, it } from 'vitest';
import type { Project, Task } from './api';
import { TaskExperienceProposal } from './TaskExperienceProposal';
import { experienceLimits } from './task-experience-proposal';

const project: Project = {
  id: 'project', name: '项目甲', revision: 1, archived: false, created_at: '', updated_at: '',
  identity: { canonical_root: '/work/project', device: 1, inode: 2, kind: { kind: 'directory' } },
};
const task: Task = {
  id: 'task-1', project_id: project.id, goal: '检查输入数据', revision: 2, spec_revision: 1,
  acceptance_ref: 'formal-acceptance', acceptance_sequence: 9, state: 'accepted',
  created_at: '', updated_at: '',
};

it.each([
  { acceptance_ref: null },
  { acceptance_ref: ' ' },
  { acceptance_sequence: null },
  { revision: 0 },
  { spec_revision: 0 },
  { project_id: undefined },
  { project_id: 'different-project' },
])('rejects an incomplete or mismatched source with %j', change => {
  render(<TaskExperienceProposal task={{ ...task, ...change }} project={project} selectedProjectId="" language="zh" />);
  expect(screen.queryByRole('button', { name: '创建经验候选' })).toBeNull();
});

it('validates both text fields, then previews a rule with localized labels and exact-boundary Unicode text', () => {
  render(<TaskExperienceProposal task={task} project={project} selectedProjectId={project.id} language="zh" />);
  fireEvent.click(screen.getByRole('button', { name: '创建经验候选' }));
  const region = within(screen.getByRole('region', { name: '经验候选' }));
  editField(region.getByRole('combobox'), 'rule');
  const summary = region.getByRole('textbox', { name: '摘要' });
  const content = region.getByRole<HTMLTextAreaElement>('textbox', { name: '可复用内容' });
  const preview = region.getByRole('button', { name: '预览候选' });
  expect(content.value).toContain('适用条件');
  for (const invalid of ['   ', '验'.repeat(experienceLimits.summary + 1)]) {
    fireEvent.change(summary, { target: { value: invalid } });
    fireEvent.click(preview);
    expect(region.getByRole('alert')).toHaveTextContent('请输入 1 至 500 个字符的摘要。');
    expect(summary).toHaveFocus();
    expect(summary).toHaveAttribute('aria-invalid', 'true');
  }
  const maxSummary = '🧪'.repeat(experienceLimits.summary);
  fireEvent.change(summary, { target: { value: maxSummary } });
  for (const invalid of ['', '复'.repeat(experienceLimits.content + 1)]) {
    fireEvent.change(content, { target: { value: invalid } });
    fireEvent.click(preview);
    expect(region.getByRole('alert')).toHaveTextContent('请输入 1 至 4000 个字符的可复用内容。');
    expect(content).toHaveFocus();
    expect(content).toHaveAttribute('aria-invalid', 'true');
  }
  const maxContent = '复'.repeat(experienceLimits.content);
  fireEvent.change(content, { target: { value: maxContent } });
  fireEvent.click(preview);
  expect(region.getByRole('heading', { name: '候选预览' })).toHaveFocus();
  expect(region.getByText('规则')).toBeVisible();
  expect(region.getByText('项目甲 (project)')).toBeVisible();
  expect(region.getByText(maxSummary)).toBeVisible();
  expect(region.getByText(maxContent)).toBeVisible();
});

it('discards edits and scopes the next draft to the next task', () => {
  const view = render(<TaskExperienceProposal task={task} project={project} selectedProjectId={project.id} language="zh" />);
  const start = () => fireEvent.click(screen.getByRole('button', { name: '创建经验候选' }));
  start();
  fireEvent.change(screen.getByLabelText('摘要'), { target: { value: '临时编辑' } });
  fireEvent.click(screen.getByRole('button', { name: '放弃候选' }));
  expect(screen.getByRole('button', { name: '创建经验候选' })).toHaveFocus();
  start();
  expect(screen.getByLabelText('摘要')).toHaveValue(task.goal);
  fireEvent.change(screen.getByLabelText('摘要'), { target: { value: '另一个临时编辑' } });
  const next = { ...task, id: 'task-2', goal: '检查输出数据' };
  view.rerender(<TaskExperienceProposal task={next} project={project} selectedProjectId={project.id} language="zh" />);
  start();
  expect(screen.getByLabelText('摘要')).toHaveValue(next.goal);
});
