<!--- [list_success] -->
#### `autoschematic list: connector {{connector}}`

```
Found {{total}} objects, {{not_present}} of which are not present in the repo.
{% for path in paths %}
  - `{{ path }}` 
{% endfor %}
```
