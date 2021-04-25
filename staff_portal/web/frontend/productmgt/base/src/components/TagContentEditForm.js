import React from 'react';
import {BaseModalForm} from '../js/common/react.js';

export default class TagContentEditForm extends BaseModalForm {
    constructor(props) {
        super(props);
        this._input_refs['title'] = React.createRef();
        // `name` and `title` field share the same reference because `title` is required
        // by react-sortable-tree and `name` is recognized by backend service
        this._input_refs['name']  = this._input_refs['title'];
        // `id` field is loaded to this form but never modified
        this._input_refs['id']    = React.createRef();
    }

    get_field_names() {
        return ['id', 'name'];
    }

    render() {
        var submit_btn_label = this.state.addnode ? 'Create': 'Update';
        var form_title = this.state.addnode ? 'Create new tag': 'Update existing tag';
        form_title += (this.state.addnode && !this.state.addchild ? 
            ' at root level'
            :' under the node (index '+ this.state.node_key_path +')'
        );
        let user_form = 
            <div className="row">
              <div className="col-lg-6">
                <div className="mb-3">
                  <label className="form-label" style={{textAlign:'left'}}> Tag Title </label>
                  <input type="text"   id="title" className="form-control" ref={ this._input_refs['title'] } />
                  <input type="hidden" id="id"    ref={ this._input_refs['id'] } />
                </div>
              </div>
            </div>
        ;
        var elements = {
            form_title:form_title,  user_form:user_form, submit_btn_label:submit_btn_label,
            class_name: {
                modal: {
                    frame : ['modal', 'modal-blur', 'fade'],
                    header: ['modal-header'],
                    title : ['modal-title'],
                    body  : ['modal-body'],
                    footer: ['modal-footer'],
                    btn_cancel: ["btn btn-link link-secondary"],
                    btn_apply : ["btn btn-primary ml-auto"],
                }
            },
        };
        var renderred = BaseModalForm.prototype.render.call(this, elements);
        return renderred;
    } // end of render()
};


