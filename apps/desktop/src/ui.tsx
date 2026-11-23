import {
  ActionIcon, Box, Button as MantineButton, Checkbox, Combobox, Input as MantineInput,
  NativeSelect, Textarea as MantineTextarea, UnstyledButton,
} from '@mantine/core';
import type { ComponentPropsWithRef } from 'react';

export { Code, Fieldset, Table, Title } from '@mantine/core';

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

export function Select({ size: _size, className, ...props }: ComponentPropsWithRef<'select'>) {
  return <NativeSelect size="sm" className={className} {...props} />;
}

export function Textarea({ className, ...props }: ComponentPropsWithRef<'textarea'>) {
  return <MantineTextarea size="sm" resize="vertical" classNames={{ input: className }}
    error={props['aria-invalid'] === true}
    attributes={{ input: { 'aria-describedby': props['aria-describedby'], 'aria-invalid': props['aria-invalid'] } }}
    {...props} />;
}

const fieldWrapper = { component: 'label', className: 'desktop-field' };
const fieldCaption = { className: 'desktop-field-caption' };

export function LabeledSelect({ caption, multiple, size, ...props }: ComponentPropsWithRef<'select'> & { caption: string }) {
  return <MantineInput component="select" size="sm" multiple={multiple} multiline={multiple}
    inputSize={size?.toString()} pointer={!multiple}
    wrapperProps={fieldWrapper} leftSection={caption} leftSectionProps={fieldCaption}
    rightSection={multiple ? undefined : <Combobox.Chevron size="sm" />}
    rightSectionProps={{ className: "desktop-field-chevron" }}
    {...props} />;
}

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
