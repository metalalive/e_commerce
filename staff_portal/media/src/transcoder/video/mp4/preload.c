#include <assert.h>
#include <string.h>
#include <unistd.h>
#include <h2o/memory.h>

#include "transcoder/video/mp4.h"

static void atfp_mp4__switch_to_fchunk__for_atom_header_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result);
static void atfp_mp4__write_srcfile_atom_to_local_tmpbuf_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nwrite);
static void atfp_mp4__write_srcfile_mdat_header_to_local_tmpbuf_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nwrite);


uint8_t atfp_mp4_validate_atom_type(char *type)
{
    uint8_t is_free = strncmp(type, "free", sizeof(uint32_t)) == 0;
    uint8_t is_ftyp = strncmp(type, "ftyp", sizeof(uint32_t)) == 0;
    uint8_t is_moov = strncmp(type, "moov", sizeof(uint32_t)) == 0;
    uint8_t is_mdat = strncmp(type, "mdat", sizeof(uint32_t)) == 0;
    return (is_free|is_ftyp|is_moov|is_mdat);
}

static void atfp_mp4__read_input_atom_header_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nread)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *)cfg->cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG];
    atfp_t *processor = & mp4proc -> super;
    json_t *err_info = processor->data.error;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        asa_cfg_t *storage = processor->data.src.storage.config;
        if(cfg->op.read.dst_sz == nread) {
            if(nread < sizeof(mp4_atom)) {
                // there must be part of bytes preserved and read from previous
                // file chunk, combine it with currently read bytes.
                cfg->op.read.dst -= mp4proc->internal.nread_prev_chunk; // will point back later
                cfg->op.read.dst_max_nbytes += mp4proc->internal.nread_prev_chunk;
                nread += mp4proc->internal.nread_prev_chunk;
            }
            assert(nread == sizeof(mp4_atom)); // TODO, responsiveness
            mp4_atom  a0 = {0}, a1 = {0};
            memcpy((char *)&a0, cfg->op.read.dst, nread);
            a1 = (mp4_atom) {.size=htobe32(a0.size), .type=a0.type}; // TODO, check CPU endianness
            uint8_t is_mdat = strncmp((char *)&a1.type, "mdat", sizeof(uint32_t)) == 0;
            if(is_mdat) { // skip, preserve read bytes, jump to next atom
                mp4proc->internal.mdat.header = a0; // preserve `mdat` atom header, and jump to the position later
                mp4proc->internal.mdat.fchunk_seq = processor->filechunk_seq.curr;
                mp4proc->internal.mdat.pos  = cfg->op.seek.pos;
                mp4proc->internal.mdat.size = a1.size - sizeof(a1);
                size_t  next_atomhead_pos = mp4proc->internal.mdat.pos + mp4proc->internal.mdat.size;
                int next_chunk_seq = atfp_estimate_src_filechunk_idx(processor->data.spec,
                        processor->filechunk_seq.curr, &next_atomhead_pos);
                cfg->op.read.offset = next_atomhead_pos;
                cfg->op.read.dst_sz = sizeof(mp4_atom);
                cfg->op.read.cb = atfp_mp4__read_input_atom_header_cb;
                if(next_chunk_seq == processor->filechunk_seq.curr) { // next atom can be fetched in the same file chunk
                    result = storage->ops.fn_read(cfg);
                } else { // the app has to switch to subsequent filechunk first, then read from the position
                    mp4proc->internal.nread_prev_chunk = 0;
                    result = atfp_switch_to_srcfile_chunk(processor, next_chunk_seq,
                        atfp_mp4__switch_to_fchunk__for_atom_header_cb);
                }
            } else if (atfp_mp4_validate_atom_type((char *)&a1.type)) { // write valid atom header to local temp buffer.
                mp4proc->internal.curr_atom.size = a1.size;
                mp4proc->internal.curr_atom.nbytes_copied = 0;
                // TODO,  build software pipeline stages, e.g. raad-from-input stage and
                //  write-to-local-buffer stage may be able to work at the same time.
                asa_op_localfs_cfg_t *asa_cfg_local = &mp4proc->local_tmpbuf_handle;
                asa_cfg_local->super.op.write.cb = atfp_mp4__write_srcfile_atom_to_local_tmpbuf_cb;
                asa_cfg_local->super.op.write.src = cfg->op.read.dst;
                asa_cfg_local->super.op.write.src_sz = nread;
                asa_cfg_local->super.op.write.src_max_nbytes = nread;
                asa_cfg_local->super.op.write.offset = APP_STORAGE_USE_CURRENT_FILE_OFFSET;
                result = app_storage_localfs_write((asa_op_base_cfg_t *)asa_cfg_local);
            } else { // invalid atom type
                json_object_set_new(err_info, "transcoder", json_string("[mp4] invalid atom type"));
            }
            if(result != ASTORAGE_RESULT_ACCEPT)
                json_object_set_new(err_info, "storage", json_string("[mp4] failed to issue operation after atom header is read"));
        } else { // cfg->op.read.dst_sz > nread, reach end of current file chunk
            int next_chunk_seq = -1;
            cfg->op.read.offset = 0; // reset the offset for next file chunk
            processor->filechunk_seq.eof_reached = 0x1;
            // check whether next atom header sits between two file chunks , or whether
            // we are reaching the end of the final file chunk.
            result = atfp_switch_to_srcfile_chunk(processor, next_chunk_seq,
                           atfp_mp4__switch_to_fchunk__for_atom_header_cb);
            if(result == ASTORAGE_RESULT_ACCEPT) {
                mp4proc->internal.nread_prev_chunk = nread;
            } else if(result == ASTORAGE_RESULT_DATA_ERROR) {// currently it reaches the end of final file chunk 
                if(nread == 0) { // at the moment, all atoms excluding `mdat` are stored to local temp buffer
                    asa_op_localfs_cfg_t *asa_cfg_local = &mp4proc->local_tmpbuf_handle;
                    asa_cfg_local->super.op.write.cb = atfp_mp4__write_srcfile_mdat_header_to_local_tmpbuf_cb;
                    asa_cfg_local->super.op.write.src = (char *)&mp4proc->internal.mdat.header;
                    asa_cfg_local->super.op.write.src_sz = sizeof(mp4_atom);
                    asa_cfg_local->super.op.write.src_max_nbytes = sizeof(mp4_atom);
                    asa_cfg_local->super.op.write.offset = APP_STORAGE_USE_CURRENT_FILE_OFFSET;
                    result = app_storage_localfs_write((asa_op_base_cfg_t *)asa_cfg_local);
                    if(result != ASTORAGE_RESULT_ACCEPT)
                        json_object_set_new(err_info, "storage", json_string("[mp4] failed to issue write operation for mdat atom"));
                } else {
                    json_object_set_new(err_info, "storage", json_string("read corrupted atom header in mp4 input"));
                }
            } else { // failed to close for unknown reason
                json_object_set_new(err_info, "storage", json_string("failed to close current file chunk"));
            }
        }
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to read atom header from mp4 input"));
    }
    if(json_object_size(err_info) > 0) 
        mp4proc->internal.callback.preload_stream_info(mp4proc) ;
} // end of atfp_mp4__read_input_atom_header_cb


static void atfp_mp4__read_input_atom_body_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nread)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *)cfg->cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG];
    atfp_t *processor = & mp4proc -> super;
    json_t *err_info = processor->data.error;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_localfs_cfg_t *asa_cfg_local = &mp4proc->local_tmpbuf_handle;
        asa_cfg_local->super.op.write.src = cfg->op.read.dst;
        asa_cfg_local->super.op.write.src_sz = nread;
        asa_cfg_local->super.op.write.src_max_nbytes = nread;
        asa_cfg_local->super.op.write.offset = APP_STORAGE_USE_CURRENT_FILE_OFFSET;
        asa_cfg_local->super.op.write.cb = atfp_mp4__write_srcfile_atom_to_local_tmpbuf_cb;
        processor->filechunk_seq.eof_reached = cfg->op.read.dst_sz > nread;
        result = app_storage_localfs_write((asa_op_base_cfg_t *)asa_cfg_local);
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("[mp4] failed to issue write operation for atom body"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to read atom body from mp4 input"));
    }
    if(json_object_size(err_info) > 0) 
        mp4proc->internal.callback.preload_stream_info(mp4proc) ;
} // end of atfp_mp4__read_input_atom_body_cb


static void atfp_mp4__switch_to_fchunk__for_atom_header_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *)cfg->cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG];
    atfp_t *processor = & mp4proc -> super;
    json_t *err_info = processor->data.error;
    assert(cfg == processor->data.src.storage.handle);
    if(result == ASTORAGE_RESULT_COMPLETE) {
        size_t expect_nread = sizeof(mp4_atom) - mp4proc->internal.nread_prev_chunk;
        cfg->op.read.dst += mp4proc->internal.nread_prev_chunk; // will point back later
        cfg->op.read.dst_max_nbytes -= mp4proc->internal.nread_prev_chunk;
        cfg->op.read.cb = atfp_mp4__read_input_atom_header_cb;
        cfg->op.read.dst_sz = expect_nread;
        asa_cfg_t *storage = processor->data.src.storage.config;
        result = storage->ops.fn_read(cfg);
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("failed to issue read operation to mp4 input"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to open next source file chunk"));
    }
    if(json_object_size(err_info) > 0) 
        mp4proc->internal.callback.preload_stream_info(mp4proc) ;
} // end of atfp_mp4__switch_to_fchunk__for_atom_header_cb


static void atfp_mp4__switch_fchunk__postpone_atom_read_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *)cfg->cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG];
    atfp_t *processor = & mp4proc -> super;
    json_t *err_info = processor->data.error;
    assert(cfg == processor->data.src.storage.handle);
    if(result == ASTORAGE_RESULT_COMPLETE) {
        asa_cfg_t  *storage  = processor->data.src.storage.config;
        result = storage->ops.fn_read(cfg);
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("[mp4] failed to issue next read operation for atom"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to open next source file chunk"));
    }
    if(json_object_size(err_info) > 0) 
        mp4proc->internal.callback.preload_stream_info(mp4proc) ;
} // end of atfp_mp4__switch_fchunk__postpone_atom_read_cb


static void atfp_mp4__write_srcfile_atom_to_local_tmpbuf_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nwrite)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *) H2O_STRUCT_FROM_MEMBER(atfp_mp4_t, local_tmpbuf_handle, cfg);
    atfp_t     *processor = & mp4proc -> super;
    json_t     *err_info = processor->data.error;
    asa_cfg_t  *storage  = processor->data.src.storage.config;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        size_t nbytes_tot_atom = mp4proc->internal.curr_atom.size;
        size_t nbytes_copied   = mp4proc->internal.curr_atom.nbytes_copied;
        nbytes_copied += nwrite;
        mp4proc->internal.curr_atom.nbytes_copied = nbytes_copied;
        cfg = processor->data.src.storage.handle;
        if(nbytes_tot_atom == nbytes_copied) { // end of current atom reached, load next atom
            cfg->op.read.dst_sz = sizeof(mp4_atom);
            cfg->op.read.cb = atfp_mp4__read_input_atom_header_cb;
        } else if(nbytes_tot_atom > nbytes_copied) { // copy rest of atom body
            size_t nbytes_max_rdbuf = cfg->op.read.dst_max_nbytes;
            size_t nbytes_unread = nbytes_tot_atom - nbytes_copied;
            size_t expect_nread = (nbytes_max_rdbuf <= nbytes_unread) ? nbytes_max_rdbuf: nbytes_unread;
            cfg->op.read.dst_sz = expect_nread;
            cfg->op.read.cb = atfp_mp4__read_input_atom_body_cb;
        }
        if(processor->filechunk_seq.eof_reached) {
            int nxt_fchunk = -1;
            cfg->op.read.offset = 0;
            result = atfp_switch_to_srcfile_chunk(processor, nxt_fchunk,
                           atfp_mp4__switch_fchunk__postpone_atom_read_cb);
        } else {
            // do not use APP_STORAGE_USE_CURRENT_FILE_OFFSET , it will start reading from last read pointer of the opened file
            cfg->op.read.offset = cfg->op.seek.pos;
            result = storage->ops.fn_read(cfg);
        }
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("[mp4] failed to issue next read operation for atom"));
    } else { // write error
        json_object_set_new(err_info, "storage", json_string("failed to write atom to local temp buffer"));
    }
    if(json_object_size(err_info) > 0) 
        mp4proc->internal.callback.preload_stream_info(mp4proc) ;
} // end of atfp_mp4__write_srcfile_atom_to_local_tmpbuf_cb


static void atfp_mp4__write_srcfile_mdat_header_to_local_tmpbuf_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nwrite)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *) H2O_STRUCT_FROM_MEMBER(atfp_mp4_t, local_tmpbuf_handle, cfg);
    atfp_t   *processor = & mp4proc -> super;
    json_t   *err_info  = processor->data.error;
    if(result != ASTORAGE_RESULT_COMPLETE) { // write error
        json_object_set_new(err_info, "storage", json_string("failed to write atom to local temp buffer"));
    }
    mp4proc->internal.callback.preload_stream_info(mp4proc) ;
} // end of atfp_mp4__write_srcfile_mdat_header_to_local_tmpbuf_cb


ASA_RES_CODE  atfp_mp4__preload_stream_info (atfp_mp4_t *mp4proc, void (*cb)(atfp_mp4_t *))
{
    size_t expect_nread = sizeof(mp4_atom);
    atfp_t *processor = & mp4proc -> super;
    asa_op_base_cfg_t *cfg = processor->data.src.storage.handle;
    cfg->op.read.cb = atfp_mp4__read_input_atom_header_cb;
    cfg->op.read.dst_sz = expect_nread;
    cfg->op.read.offset = 0; // point back to beginning of file, then read the first few byte
    mp4proc->internal.callback.preload_stream_info = cb;
    asa_cfg_t *storage = processor->data.src.storage.config;
    return storage->ops.fn_read(cfg);
} // end of atfp_mp4__preload_stream_info


ASA_RES_CODE  atfp_mp4__preload_initial_packets (atfp_mp4_t *mp4proc, size_t num, void (*cb)(atfp_mp4_t *))
{
    asa_op_localfs_cfg_t *local_tmpbuf = (asa_op_localfs_cfg_t *)&mp4proc->local_tmpbuf_handle;
    // TODO, load first few frames, then avformat_find_stream_info()
    lseek(local_tmpbuf->file.file, 0, SEEK_SET);
    cb(mp4proc);
    return ASTORAGE_RESULT_ACCEPT;
} // end of atfp_mp4__preload_initial_packets
