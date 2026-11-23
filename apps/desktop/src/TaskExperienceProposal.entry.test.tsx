import { choosePickerOption } from './test-picker';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { App } from './App';
import type { Project, Task } from './api';

const { ipc } = vi.hoisted(() => ({ ipc: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: ipc }));

const stamp = '2026-11-19T00:00:00Z';
const project = (id: string, name: string): Project => ({
  id, name, revision: 1, archived: false, created_at: stamp, updated_at: stamp,
  identity: { canonical_root: `/work/${id}`, device: 1, inode: 2, kind: { kind: 'directory' } },
});
const atlas = project('project-atlas', 'Atlas');
const boreal = project('project-boreal', 'Boreal');
const accepted: Task = {
  id: 'task-atlas', project_id: atlas.id, goal: 'Validate the Atlas schema', state: 'accepted',
  revision: 7, spec_revision: 3, acceptance_ref: 'acceptance:atlas', acceptance_sequence: 11,
  created_at: stamp, updated_at: stamp,
};
const other: Task = {
  ...accepted, id: 'task-boreal', project_id: boreal.id, goal: 'Boreal private source',
  acceptance_ref: 'acceptance:boreal', acceptance_sequence: 12,
};
const emptyReads = [
  'connections_list', 'sessions_list', 'workspaces_list',
  'data_workspaces_list', 'data_workspace_previews_list',
];
const reads = ['system_snapshot', 'events_subscribe', 'projects_list', 'client_recovery_list', ...emptyReads];

let tasks: Task[];

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem('agentbaton.language', 'en');
  tasks = [accepted, other];
  ipc.mockReset();
  ipc.mockImplementation(async (command: string) => {
    const response = (result: unknown) => ({ result, warnings: [] });
    switch (command) {
      case 'system_snapshot': return response({ tasks, sequence: 20, api_version: 1, database_schema_version: 19 });
      case 'events_subscribe': return response({ events: [], next_cursor: 20, sequence: 20, gap: false });
      case 'projects_list': return response([atlas, boreal]);
      case 'client_recovery_list': return response({ entries: [] });
      default:
        if (emptyReads.includes(command)) return response([]);
        throw new Error(`Unexpected IPC command ${command}`);
    }
  });
});

async function openDetails(task: Task = accepted) {
  fireEvent.click(await screen.findByRole('button', { name: `Open task: ${task.goal}` }));
  fireEvent.click(screen.getByRole('tab', { name: 'Details' }));
}

function editor() {
  return within(screen.getByRole('region', { name: 'Experience candidate' }));
}

async function openEditor() {
  await userEvent.click(await screen.findByRole('button', { name: 'Create experience candidate' }));
}

function previewCandidate() {
  fireEvent.click(editor().getByRole('button', { name: 'Preview candidate' }));
}

it('creates, previews and revises accepted-task experience from the normal task card with local text only', async () => {
  render(<App />);
  await openDetails();
  await openEditor();
  const candidate = editor();
  expect(candidate.getByRole('combobox', { name: 'Candidate type' })).toHaveFocus();
  expect(candidate.getByRole('textbox', { name: 'Summary' })).toHaveValue(accepted.goal);
  await choosePickerOption(userEvent, candidate.getByRole('combobox'), 'template');
  const content = candidate.getByRole<HTMLTextAreaElement>('textbox', { name: 'Reusable content' });
  expect(content.value).toContain('Acceptance checks');
  fireEvent.change(content, { target: { value: 'x'.repeat(4001) } });
  previewCandidate();
  expect(candidate.getByRole('alert')).toHaveTextContent('Enter reusable content of 1–4000 characters.');
  expect(content).toHaveFocus();
  expect(candidate.queryByRole('heading', { name: 'Candidate preview' })).toBeNull();
  const plainText = '<tool name="register">review this text</tool>';
  fireEvent.change(content, { target: { value: plainText } });
  previewCandidate();
  expect(candidate.getByRole('heading', { name: 'Candidate preview' })).toHaveFocus();
  for (const text of ['Template', 'Atlas (project-atlas)', 'task-atlas', '7', '3', 'acceptance:atlas', '11', accepted.goal, plainText]) {
    expect(candidate.getByText(text, { exact: true })).toBeVisible();
  }
  expect(candidate.queryByText(/boreal/i)).toBeNull();
  expect(candidate.getByText(plainText).querySelector('tool')).toBeNull();

  await userEvent.click(candidate.getByRole('button', { name: 'Edit candidate' }));
  expect(candidate.getByLabelText('Reusable content')).toHaveValue(plainText);
  fireEvent.change(candidate.getByLabelText('Summary'), { target: { value: 'Reviewed Atlas checks' } });
  previewCandidate();
  expect(candidate.getByText('Reviewed Atlas checks')).toBeVisible();
  await userEvent.click(candidate.getByRole('button', { name: 'Discard candidate' }));
  expect(candidate.queryByRole('heading', { name: 'Candidate preview' })).toBeNull();
  expect(candidate.getByRole('button', { name: 'Create experience candidate' })).toHaveFocus();
  await openEditor();
  expect(candidate.getByLabelText('Summary')).toHaveValue(accepted.goal);
  expect(ipc.mock.calls.map(([command]) => command).filter(command => !reads.includes(command))).toEqual([]);
  expect(Object.keys(localStorage)).toEqual(['agentbaton.language']);
  expect(sessionStorage.length).toBe(0);
});

it('keeps candidate creation out of an unaccepted task even when its projection carries a reference', async () => {
  const running = { ...accepted, state: 'running' };
  tasks = [running];
  render(<App />);
  await openDetails(running);
  expect(screen.getByRole('tab', { name: 'Details' })).toHaveAttribute('aria-selected', 'true');
  expect(screen.queryByRole('region', { name: 'Experience candidate' })).toBeNull();
});

it('removes the candidate when refresh can no longer verify its acceptance reference', async () => {
  render(<App />);
  await openDetails();
  await openEditor();
  previewCandidate();
  expect(editor().getByText('acceptance:atlas')).toBeVisible();
  tasks = [{ ...accepted, acceptance_ref: null }];
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  await screen.findByText('Invalid accepted Task projection');
  expect(screen.queryByRole('region', { name: 'Experience candidate' })).toBeNull();
});

it.each([
  ['task revision', { revision: 8 }, '8'],
  ['specification revision', { spec_revision: 4 }, '4'],
  ['acceptance reference', { acceptance_ref: 'acceptance:replacement' }, 'acceptance:replacement'],
  ['acceptance sequence', { acceptance_sequence: 14 }, '14'],
] as const)('clears the active candidate when the %s changes', async (_, change, expected) => {
  render(<App />);
  await openDetails();
  await openEditor();
  fireEvent.change(editor().getByLabelText('Summary'), { target: { value: 'Old draft' } });
  previewCandidate();
  tasks = [{ ...accepted, ...change }];
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  await screen.findByRole('button', { name: 'Create experience candidate' });
  expect(editor().queryByText('Old draft')).toBeNull();
  await openEditor();
  previewCandidate();
  expect(editor().getByText(expected, { exact: true })).toBeVisible();
});

it('clears a draft on a project selector change while retaining the existing task details', async () => {
  render(<App />);
  await openDetails();
  await openEditor();
  fireEvent.change(editor().getByLabelText('Summary'), { target: { value: 'Private Atlas draft' } });
  previewCandidate();
  await userEvent.click(screen.getByRole('button', { name: 'Projects & workspaces' }));
  const dialog = within(screen.getByRole('region', { name: 'Projects & workspaces' }));
  await userEvent.click(dialog.getByRole('button', { name: /Boreal.*\/work\/project-boreal/i }));
  await userEvent.click(dialog.getByRole('button', { name: 'Back' }));
  expect(screen.getByRole('heading', { name: accepted.goal })).toBeVisible();
  expect(screen.queryByRole('region', { name: 'Experience candidate' })).toBeNull();

  await userEvent.click(screen.getByRole('button', { name: 'Back to tasks' }));
  await openDetails(other);
  await openEditor();
  previewCandidate();
  expect(editor().getByText('Boreal (project-boreal)')).toBeVisible();
  expect(editor().queryByText(/Atlas/)).toBeNull();
  expect(editor().getByText('acceptance:boreal')).toBeVisible();
  expect(ipc.mock.calls.map(([command]) => command).filter(command => !reads.includes(command))).toEqual([]);
});
