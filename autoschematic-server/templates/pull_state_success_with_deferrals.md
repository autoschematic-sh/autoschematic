<!--- [pull_state_success_with_deferrals] -->
#### `{{success_emoji}} autoschematic pull-state: success ({{deferred_count}} deferred)`

Successfully pulled-state for {{object_count}} objects, {{import_count}} of which had state differences to import.

{% if deferred_count == 1%}
1 object was
{% else %}
{{deferred_count}} objects were
{% endif %}
 deferred waiting for the following outputs:
<details>
<summary>

</summary>

{% for key in output_keys %}

`{{key}}`


{% endfor %}
</details>
