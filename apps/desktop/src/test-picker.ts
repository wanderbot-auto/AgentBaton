import { fireEvent, within } from '@testing-library/react';

function findOption(control: HTMLElement, value: string) {
  const list = document.getElementById(control.getAttribute('aria-controls') ?? '');
  if (!list) throw new Error('Picker did not open a listbox');
  const option = within(list).getAllByRole('option')
    .find(item => item.getAttribute('value') === ':' + value);
  if (!option) throw new Error('Picker does not offer value ' + value);
  return option;
}

export async function choosePickerOption(
  user: { click: (element: Element) => Promise<void> },
  control: HTMLElement,
  value: string | string[],
) {
  for (const entry of typeof value === 'string' ? [value] : value) {
    if (control.getAttribute('aria-expanded') !== 'true') await user.click(control);
    await user.click(findOption(control, entry));
  }
}

export function openPicker(control: HTMLElement) {
  if (control.getAttribute('aria-expanded') !== 'true') {
    fireEvent.mouseDown(control);
    fireEvent.click(control);
  }
}

export function clickPickerOption(control: HTMLElement, value: string) {
  openPicker(control);
  fireEvent.click(findOption(control, value));
}

export function editField(control: HTMLElement, value: string | number) {
  if (control.getAttribute('role') === 'combobox') clickPickerOption(control, String(value));
  else fireEvent.change(control, { target: { value } });
}
