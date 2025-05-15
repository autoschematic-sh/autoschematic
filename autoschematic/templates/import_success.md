<!--- [import_success] -->
#### `{{success_emoji}} autoschematic import: Success!`

```
Found {{total_count}} objects, {{imported_count}} of which were imported into the repo.
{% for path in paths %}
  - `{{ path }}` 
{% endfor %}
```
