import {
  ActionIcon, Box, Button as MantineButton, Checkbox, Fieldset as MantineFieldset, Input as MantineInput,
  MultiSelect as MantineMultiSelect, Select as MantineSelect, Textarea as MantineTextarea,
  UnstyledButton, type FieldsetProps, type MultiSelectProps, type SelectProps,
} from '@mantine/core';
import { useUncontrolled } from '@mantine/hooks';
import { Children, createContext, isValidElement, useContext, type ComponentPropsWithRef, type KeyboardEvent, type ReactNode } from 'react';

export { Code, Table, Title } from '@mantine/core';

const quietButtonClasses = new Set([
  'nav-item', 'new-button', 'task-card', 'text-button',
  'management-list-row', 'management-attention', 'record-button',
]);
const iconButtonClasses = new Set(['icon-button', 'send-button', 'task-notifications-trigger']);

export function Button({ type = 'submit', className, ...props }: ComponentPropsWithRef<'button'>) {
  const classes = className?.split(/\s+/) ?? [];
  const primary = classes.includes('primary');
  if (classes.some(name => iconButtonClasses.has(name))) {
    return <ActionIcon type={type} variant={primary ? 'filled' : 'subtle'} size="md" className={className} {...props} />;
  }
  if (classes.some(name => quietButtonClasses.has(name))) {
    return <UnstyledButton type={type} className={className} {...props} />;
  }
  return <MantineButton type={type} variant={primary ? 'filled' : 'default'} size="sm"
    className={className} {...props} />;
}

export function Input({ size: _size, type, className, ...props }: ComponentPropsWithRef<'input'>) {
  if (type === 'checkbox') {
    return <Checkbox size="sm" classNames={{ input: className }} {...props} />;
  }
  return <MantineInput type={type} size="sm" classNames={{ input: className }}
    attributes={{ input: { 'aria-describedby': props['aria-describedby'] } }} {...props} />;
}

type ChoiceProps = { value?: string | number; disabled?: boolean; children: ReactNode };

// Option declarations keep choices next to the feature that supplies them.
// The picker converts them to Mantine data and renders the dropdown in the WebView.
export function SelectOption(_props: ChoiceProps) {
  return null;
}

// Mantine indexes option values in an object. Keep record IDs outside its
// prototype property names, and submit original IDs to forms and callers.
function optionKey(value: string) {
  return ':' + value;
}

function selectData(children: ReactNode) {
  const choices = new Map<string, { value: string; label: string; disabled?: boolean }>();
  for (const child of Children.toArray(children)) {
    if (!isValidElement<ChoiceProps>(child) || child.type !== SelectOption) continue;
    const label = Children.toArray(child.props.children).join('').replace(/\s+/g, ' ').trim();
    const value = optionKey(String(child.props.value ?? label));
    const existing = choices.get(value);
    choices.set(value, existing
      ? { ...existing, disabled: true }
      : { value, label, disabled: child.props.disabled });
  }
  return Array.from(choices.values());
}

const FieldsetDisabled = createContext(false);

export function Fieldset({ children, disabled, ...props }: FieldsetProps) {
  const inheritedDisabled = useContext(FieldsetDisabled);
  return <MantineFieldset disabled={disabled} {...props}>
    <FieldsetDisabled value={Boolean(disabled || inheritedDisabled)}>{children}</FieldsetDisabled>
  </MantineFieldset>;
}

type DesktopSelectProps = Omit<SelectProps, 'data' | 'onChange'> & {
  children: ReactNode;
  onChange?: (value: string) => void;
  ref?: ComponentPropsWithRef<'input'>['ref'];
};

const pickerPopover = { transitionProps: { duration: 0 } };

function keepEscapeInPicker(event: KeyboardEvent<HTMLInputElement>) {
  if (event.key === 'Escape' && event.currentTarget.getAttribute('aria-expanded') === 'true') {
    event.preventDefault();
    event.stopPropagation();
  }
}

export function Select({
  children, disabled, defaultValue, value, onChange, required, placeholder, name, form, ...props
}: DesktopSelectProps) {
  const fieldsetDisabled = useContext(FieldsetDisabled);
  const unavailable = disabled || fieldsetDisabled;
  const [selected, setSelected] = useUncontrolled({ value: value ?? undefined, defaultValue: defaultValue ?? undefined, finalValue: '', onChange });
  const choices = selectData(children);
  const empty = choices.find(choice => choice.value === optionKey(''));

  return <><MantineSelect className="desktop-select" size="sm" comboboxProps={pickerPopover} allowDeselect={false}
    data={required ? choices.filter(choice => choice !== empty) : choices} disabled={unavailable}
    required={required} placeholder={placeholder ?? (required ? empty?.label : undefined)}
    value={required && !selected ? null : optionKey(selected ?? '')}
    data-picker-count={required && empty ? choices.length - 1 : choices.length}
    data-picker-value={selected ?? ''} onKeyDown={keepEscapeInPicker}
    onChange={next => { if (!unavailable) setSelected(next?.slice(1) ?? ''); }} {...props} />
    {name && <Box component="input" type="hidden" name={name} form={form} value={selected} disabled={unavailable} />}
  </>;
}

type DesktopMultiSelectProps = Omit<MultiSelectProps, 'data' | 'value' | 'defaultValue'> & {
  children: ReactNode;
  value: string[];
};

export function MultiSelect({ children, disabled, value, onChange, ...props }: DesktopMultiSelectProps) {
  const fieldsetDisabled = useContext(FieldsetDisabled);
  const unavailable = disabled || fieldsetDisabled;
  return <MantineMultiSelect className="desktop-select" size="sm" comboboxProps={pickerPopover} searchable placeholder="—"
    data={selectData(children)} disabled={unavailable} value={value.map(optionKey)}
    onKeyDown={keepEscapeInPicker} data-picker-value={JSON.stringify(value)}
    onChange={next => { if (!unavailable) onChange?.(next.map(key => key.slice(1))); }} {...props} />;
}

export function Textarea({ className, ...props }: ComponentPropsWithRef<'textarea'>) {
  return <MantineTextarea size="sm" resize="vertical" classNames={{ input: className }}
    error={props['aria-invalid'] === true}
    attributes={{ input: { 'aria-describedby': props['aria-describedby'], 'aria-invalid': props['aria-invalid'] } }}
    {...props} />;
}

const fieldWrapper = { component: 'label', className: 'desktop-field' };
const fieldCaption = { className: 'desktop-field-caption' };

export function LabeledTextarea({ caption, ...props }: ComponentPropsWithRef<'textarea'> & { caption: string }) {
  return <MantineInput component="textarea" size="sm" multiline
    wrapperProps={fieldWrapper} leftSection={caption} leftSectionProps={fieldCaption} {...props} />;
}

export function Disclosure(props: ComponentPropsWithRef<'details'>) {
  return <Box component="details" {...props} />;
}

export function DisclosureLabel(props: ComponentPropsWithRef<'summary'>) {
  return <UnstyledButton component="summary" {...props} />;
}

export function Form(props: ComponentPropsWithRef<'form'>) {
  return <Box component="form" {...props} />;
}
