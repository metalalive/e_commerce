import * as toolkit from "/static/js/toolkit.js";
// TODO, deprecated, will use table-grid library instead to render backend data

function evt_order_change(evt) {
    var dataset = evt.target.dataset;
    var props = evt.target.props;
    if(props == undefined) { return; }
    var order_field = dataset.order_field;
    if(order_field == undefined) { return; }
    if(dataset.descending == "true") {
        order_field = '-' + order_field;
        dataset.descending = "false";
    } else if(dataset.descending == "false") {
        dataset.descending = "true";
    } else if(dataset.descending == undefined) {
        return;
    }
    props.query_params.ordering = order_field;
    props.load_api_fn(props);
}

function  init_order_btns(api) {
    if(api && api.avail_order_fields && api.record_list) {
        var btn_data = {};
        var table_dom = api.record_list.querySelector("table").children[0]; // auto generated tbody
        var btns = table_dom.children[0];
        var names = api.avail_order_fields;
        var query_string = names.map((x) => "a[id="+ x +"]").join(',');
        var doms = Array.from(btns.querySelectorAll(query_string));
        for(var idx = 0; idx < names.length; idx++) {
            btn_data[names[idx]] = { props:api, onclick: evt_order_change, };
        }
        toolkit.update_doms_props(doms, btn_data);
    } else {
        console.log("[warning] init_order_btns not executed");
    }
}


export { init_order_btns, };

