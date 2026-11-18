# Dress forms

Each directory here is one dress form the studio's preview can drape on:

```
assets/dressforms/<id>/form.json        the declaration (committed)
assets/dressforms/<id>/<mesh>.obj       the mesh (NOT committed: see .gitignore)
```

**The mesh is not part of the repository.** Whoever installs a form owns its licence; `form.json` records
it, and a form whose mesh is absent is listed in the picker as "not installed". Read
[`docs/dress-forms.md`](../../docs/dress-forms.md) for the schema and the checks.
