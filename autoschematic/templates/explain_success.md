#### `autoschematic: lookin' good, captain!`

```
{% for database in databases %}
`Database {{database.0}}:`
{% for file in database.1.files %}
  - `{{ file }}` : ✅
{% endfor %}
{% endfor %}
```

