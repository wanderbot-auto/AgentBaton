import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { Form, LabeledSelect, LabeledTextarea } from './ui';

it('keeps Factory fields themed, accessible and directly bound to their labels', async () => {
  const user = userEvent.setup();
  const submitted = vi.fn();
  render(<Form onSubmit={event => {
    event.preventDefault();
    submitted(new FormData(event.currentTarget));
  }}>
    <LabeledSelect caption="Existing root Grant" name="grant">
      <option value="grant-1">Grant 1</option>
    </LabeledSelect>
    <LabeledSelect caption="Existing workspaces" name="workspaces" multiple>
      <option value="workspace-1">Workspace 1</option>
      <option value="workspace-2">Workspace 2</option>
    </LabeledSelect>
    <LabeledTextarea caption="Complete GoalPlan JSON draft" name="draft" />
  </Form>);

  const grant = screen.getByRole('combobox', { name: 'Existing root Grant' });
  const workspaces = screen.getByRole('listbox', { name: 'Existing workspaces' });
  const draft = screen.getByRole('textbox', { name: 'Complete GoalPlan JSON draft' });
  for (const field of [grant, workspaces, draft]) {
    expect(field).toHaveClass('mantine-Input-input');
    expect(field.parentElement?.tagName).toBe('LABEL');
  }

  await user.selectOptions(workspaces, ['workspace-1', 'workspace-2']);
  await user.type(draft, 'bounded plan');
  const form = draft.closest('form')!;
  form.requestSubmit();

  expect(submitted).toHaveBeenCalledOnce();
  const data: FormData = submitted.mock.calls[0][0];
  expect(data.get('grant')).toBe('grant-1');
  expect(data.getAll('workspaces')).toEqual(['workspace-1', 'workspace-2']);
  expect(data.get('draft')).toBe('bounded plan');
});
