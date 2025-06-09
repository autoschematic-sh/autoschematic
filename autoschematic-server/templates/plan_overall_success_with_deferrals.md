<!--- [plan_overall_success_with_deferrals] -->
#### `{{success_emoji}} autoschematic plan: success ({{deferred_count}} deferred)`

The plan succeeded for a subset of modified items. 

Running {{apply_command}} will carry out all of the actions detailed in the above plan reports. 



{% if deferred_count == 1%}
<details><summary> 1 object was deferred waiting for the following outputs: </summary>
{% else %}
<details><summary> {{deferred_count}} objects were deferred waiting for the following outputs: </summary>
{% endif %}
{% for key in output_keys %}

`{{key}}`

{% endfor %}
</details>
