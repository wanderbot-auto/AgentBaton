# Private browser Python preparation

Base e874b123d3e7f329a551e21922baf1a7a643548f. AB-START-001 authorizes this bounded CI environment fix; OWNER-001/002 retained. Product Provider, signing and production release remain closed.

Replace only the browser dependency step's venv creation command. Use standard EnvBuilder(symlinks=False, with_pip=True) in a fresh owned private directory, retaining pyvenv.cfg, pip and base-standard-library layout. Hold and recheck original source and new directory/file identities; harden only fixed owned, single-link, regular copied interpreter files to 0500 using file descriptors. Shared Python is read and hashed, never chmodded. Do not copy user/configuration/credential trees. Existing source, owner/mode, image-size, schema, budgets and runtime guards remain unchanged.

Use a complete dependency-step pin and exact one-command inverse before every older whole-workflow guard. Agent tests synthetic files with a mocked EnvBuilder only. Root alone creates and executes an actual venv, imports standard library and Playwright, runs the original driver guard and wrapper, then obtains new CI evidence.
