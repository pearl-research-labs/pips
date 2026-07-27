# PIP Assets

Images, diagrams, and other auxiliary files belonging to a proposal live in a
subdirectory named after it: `assets/pip-XXXX/`.

Reference them from the PIP with a relative link, for example:

```markdown
![Certificate layout](../assets/pip-0002/certificate-layout.png)
```

Such links resolve both on GitHub and on the rendered site, where these files
are served under `/assets/pip-XXXX/`.
