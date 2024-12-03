# Zephyrus Contracts

To install dependencies, it is recommended to use [Nix with Flakes](https://github.com/DeterminateSystems/nix-installer).

To enter the development shell with dependencies installed:

```bash
nix develop
```

> **Tip**
> The above command will put you in a bash shell by default.  
> Use `nix develop -c <your-fancy-shell>` if you want to keep your shell.

To build contracts and generate bindings:

```bash
just
```

To view all Just recipes:

```bash
just menu
```
