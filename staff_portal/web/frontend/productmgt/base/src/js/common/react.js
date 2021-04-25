import React from 'react';
import {get_input_dom_value, get_select_dom_value, set_input_dom_value, set_select_dom_value} from './native.js';
// Note , this file cannot be linked outside react application scope
// but this file is commonly used among severl react applications, how to deal with the issue ?


export class BaseModalForm extends React.Component {
    constructor(props) {
        super(props);
        this.state = {};
        this._user_form_ref = React.createRef();
        this._input_refs = {};
    }
        
    get_field_names() {
        return Object.keys(this._input_refs);
    }

    get_data() {
        // retrieve input data from user form in object format
        var out = {}; // {title: 'electronic sensor', subtitle:'DIY maker',};
        for(var key in this._input_refs) {
            var ref = this._input_refs[key].current;
            if(ref instanceof HTMLInputElement) {
                out[key] = get_input_dom_value(ref);
            } else if(ref instanceof HTMLSelectElement) {
                out[key] = get_select_dom_value(ref);
            } else if(ref instanceof React.Component) {
                throw Error("not implemented , to get data from react component");
            }
        }
        return out;
    }

    set_data(values) {
        for(var key in this._input_refs) {
            var value = values[key];
            var ref = this._input_refs[key].current;
            if(ref instanceof HTMLInputElement) {
                set_input_dom_value(ref, value);
            } else if(ref instanceof HTMLSelectElement) {
                set_select_dom_value(ref, value);
            } else if(ref instanceof React.Component) {
                throw Error("not implemented , to set data to react component");
            }
        }
    }

    render(elements) {
        return (
            <div className={elements.class_name.modal.frame.join(' ')}  id={this.props.form_id} tabIndex="-1" role="dialog" aria-hidden="true">
              <div className="modal-dialog modal-lg modal-dialog-centered" role="document">
                <div className="modal-content">
                  <div className={elements.class_name.modal.header.join(' ')}>
                    <h5 className={elements.class_name.modal.title.join(' ')}> {elements.form_title} </h5>
                    <button type="button" className="close" data-dismiss="modal" aria-label="Close">
                      <svg xmlns="http://www.w3.org/2000/svg" className="icon" width="24" height="24" viewBox="0 0 24 24" strokeWidth="2" stroke="currentColor" fill="none" strokeLinecap="round" strokeLinejoin="round"><path stroke="none" d="M0 0h24v24H0z"/><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                    </button>
                  </div>
                  <div className={elements.class_name.modal.body.join(' ')} ref={ this._user_form_ref } >
                    { elements.user_form }
                  </div>
                  <div className={elements.class_name.modal.footer.join(' ')}>
                    <button className={elements.class_name.modal.btn_cancel.join(' ')} data-dismiss="modal">
                      Cancel
                    </button>
                    <button className={elements.class_name.modal.btn_apply.join(' ')} data-dismiss="modal" onClick={this.state.callback_submit}>
                      <svg xmlns="http://www.w3.org/2000/svg" className="icon" width="24" height="24" viewBox="0 0 24 24" strokeWidth="2" stroke="currentColor" fill="none" strokeLinecap="round" strokeLinejoin="round"><path stroke="none" d="M0 0h24v24H0z"/><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
                      { elements.submit_btn_label }
                    </button>
                  </div>
                </div>
              </div>
            </div>
        );
    }
};

