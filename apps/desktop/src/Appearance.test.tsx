import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { expect, it, vi } from 'vitest';
import { choosePickerOption } from './test-picker';
import { Fieldset, Form, LabeledTextarea, MultiSelect, Select, SelectOption } from './ui';

it('opens Mantine dropdowns, submits the original value and keeps the textarea label contract', async () => {
  const user = userEvent.setup();
  const submitted = vi.fn();
  function FactoryFields() {
    const [workspaces, setWorkspaces] = useState<string[]>([]);
    return <Form onSubmit={event => {
      event.preventDefault();
      submitted(new FormData(event.currentTarget));
    }}>
      <Select label="Existing root Grant" name="grant">
        <SelectOption value="">Choose</SelectOption>
        <SelectOption value="constructor">Grant 1</SelectOption>
      </Select>
      <MultiSelect label="Existing workspaces" value={workspaces} onChange={setWorkspaces}>
        <SelectOption value="workspace-1">Workspace 1</SelectOption>
        <SelectOption value="workspace-2">Workspace 2</SelectOption>
      </MultiSelect>
      <LabeledTextarea caption="Complete GoalPlan JSON draft" name="draft" />
    </Form>;
  }
  const { container } = render(<FactoryFields />);
  const grant = screen.getByRole('combobox', { name: 'Existing root Grant' });
  const workspaces = screen.getByRole('combobox', { name: 'Existing workspaces' });
  const draft = screen.getByRole('textbox', { name: 'Complete GoalPlan JSON draft' });
  expect(container.querySelector('select')).toBeNull();
  expect(draft.parentElement?.tagName).toBe('LABEL');

  await user.click(grant);
  expect(screen.getByRole('listbox')).toBeVisible();
  expect(screen.getByRole('option', { name: 'Grant 1' })).toBeVisible();
  await choosePickerOption(user, grant, 'constructor');
  await choosePickerOption(user, workspaces, ['workspace-1', 'workspace-2']);
  expect(workspaces).toHaveAttribute('data-picker-value', '["workspace-1","workspace-2"]');
  await user.type(draft, 'bounded plan');
  draft.closest('form')!.requestSubmit();

  expect(submitted).toHaveBeenCalledOnce();
  const data: FormData = submitted.mock.calls[0][0];
  expect(data.get('grant')).toBe('constructor');
  expect(data.get('draft')).toBe('bounded plan');
});

it('does not open dropdowns in a disabled fieldset', async () => {
  const user = userEvent.setup();
  render(<Fieldset disabled><Select label="Scoped target"><SelectOption>Target</SelectOption></Select></Fieldset>);
  const target = screen.getByRole('combobox', { name: 'Scoped target' });
  expect(target).toBeDisabled();
  await user.click(target);
  expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
});
