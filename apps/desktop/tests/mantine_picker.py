"""Exercise Mantine pickers via the rendered combobox and its associated options."""
from playwright.sync_api import expect


def open_picker(control):
    expect(control).to_be_enabled()
    if control.get_attribute('aria-expanded') != 'true':
        control.click()
    expect(control).to_have_attribute('aria-expanded', 'true')
    identifier = control.get_attribute('aria-controls')
    assert identifier and all(char.isalnum() or char in '-_' for char in identifier)
    popup = control.page.locator(f'[id="{identifier}"][role="listbox"]')
    expect(popup).to_be_visible()
    return popup


def choose_picker_option(control, value):
    values = [value] if isinstance(value, str) else value
    for entry in values:
        options = open_picker(control).get_by_role('option')
        option_values = options.evaluate_all("(items) => items.map(item => item.getAttribute('value'))")
        key = ':' + entry
        assert option_values.count(key) == 1, 'Expected exactly one rendered option with the requested value'
        options.nth(option_values.index(key)).click()
    if control.get_attribute('aria-expanded') == 'true':
        control.press('Escape')
