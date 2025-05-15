<!--- [plan_success] -->
#### `{{success_emoji}} autoschematic plan: {{filename}}:`

{% for report in op_reports %}
---

{{ report.0 }}

<details>
<summary>Raw</summary>

```
{{ report.1 }}
```

</details>

{% endfor %}
