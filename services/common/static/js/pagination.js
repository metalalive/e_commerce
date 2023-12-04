import * as toolkit from "/static/js/toolkit.js";
// TODO, deprecated, will use table-grid library instead to render backend data


function evt_page_change(evt) {
    var dataset = evt.target.dataset;
    var props = evt.target.props;
    if(props == undefined) { return; }
    var new_page_num = Number(dataset.pagenum);
    if(props.query_params.page != new_page_num) { 
        props.query_params.page = new_page_num;
        props.load_api_fn(props);
    }
}

function evt_pagesz_change(evt) {
    var option = evt.target.selectedOptions[0];
    var props = evt.target.props;
    if(props == undefined || option == undefined) {
        return;
    }
    var new_page_sz = Number(option.value);
    if(props.query_params.page_size != new_page_sz) {
        props.default_page_size = new_page_sz;
        props.query_params.page_size = new_page_sz;
        props.query_params.page = 1;
        props.load_api_fn(props);
    }
}


function update_pagination_cb(avail_data, props) {
    // only works for django REST framework, TODO make it work on other REST API framework
    var paging_panel = props.paging_panel;
    if (!avail_data.results || !avail_data.count || !paging_panel) {
        return;
    }
    var curr_page = props.query_params.page;
    var page_size = 0;
    if(curr_page == 1) {
        page_size = avail_data.results.length;
        props.query_params.page_size = page_size; // auto adjust page size
    } else {
        page_size = props.query_params.page_size;
    }
    var final_page_num = Math.ceil(avail_data.count / page_size);
    paging_panel.querySelector('a[id=page_prev]').dataset.pagenum = Math.max(1, curr_page - 1);
    paging_panel.querySelector('a[id=page_next]').dataset.pagenum = Math.min(final_page_num, curr_page + 1);
    var pagenum_btns = paging_panel.querySelectorAll("a[id=specific_page]");
    var num_btns_avail = pagenum_btns.length - 2;
    var visible = {};
    var idx = 1;
    visible[1] = '';
    visible[final_page_num] = '';
    if(curr_page != 1 && curr_page != final_page_num) {
        visible[curr_page] = '';
        num_btns_avail--;
    }
    do {
        var pagenum = (idx++ % 2 == 0) ? (curr_page + (idx >> 1)): (curr_page - (idx >> 1));
        if((pagenum <= 0) || (visible[pagenum] != undefined)) {
            continue
        } else if(pagenum > final_page_num) {
            break;
        }
        visible[pagenum] = '';
        num_btns_avail--;
    } while(num_btns_avail > 0);
    visible = Object.keys(visible).map(x => Number(x));
    visible = visible.sort((a,b) => { return a - b;});
    for(idx = 0; idx < visible.length; idx++) {
        pagenum_btns[idx].parentNode.classList.remove('active');
        pagenum_btns[idx].parentNode.hidden = false;
        pagenum_btns[idx].innerText = visible[idx];
        pagenum_btns[idx].dataset.pagenum = visible[idx];
        if(curr_page == visible[idx]) {
            pagenum_btns[idx].parentNode.classList.add('active');
        }
    }
    for( ;idx < pagenum_btns.length; idx++) {
        pagenum_btns[idx].parentNode.hidden = true;
    }
} // end of update_pagination_cb



function init_paging_panel(new_dom, api) {
    var btn_data = {};
    var links = Array.from(new_dom.querySelectorAll("a"));
    var select_elm = new_dom.querySelector("select");
    // update pagination panel again after API data is loaded
    api.paging_panel = new_dom;
    btn_data['page_prev']     = {props: api, onclick:evt_page_change };
    btn_data['page_next']     = {props: api, onclick:evt_page_change };
    btn_data['specific_page'] = {props: api, onclick:evt_page_change };
    btn_data['page_size']     = {props: api, onchange: evt_pagesz_change };
    links.push(select_elm);
    toolkit.update_doms_props(links, btn_data);
}


export { update_pagination_cb, init_paging_panel, };

