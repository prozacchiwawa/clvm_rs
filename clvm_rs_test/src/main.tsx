// main.tsx
import * as React from 'react'
import { render } from 'react-dom'
import { createElmishComponent } from '@ts-elmish/react'
import { hexlify, unhexlify } from 'binascii'

// Our callback needs the 'dispatch' function to send a message.
// We'll make our sender functions take it and yield a packaged
// up function that actually sends the message.
//
// Initially i was expecting callbacks to be wrapped in this kind
// of function automatically, but they aren't.
function sender(dispatch,f) {
    return (e) => dispatch(f(e));
}

// Make a component that does something.
// note: although you control the state (left side of the
// init "tuple"), a "dispatch" function is added to it by the
// framework, thus the state type is Record<String,any>
const App = createElmishComponent({
    init: (args) => {
        console.log('args',args);
        return [{source:"", result:"", clvm_rs: args.clvm_rs}, []];
    },
    update: (state, a) => {
        console.log('update',a);
        let action = a[0] as any;
        // Make a new state without mutation using the message content
        // action is the list given below.

        console.log(action);
        if (action.input !== undefined) {
            console.log('input', action.input);
            return [{
                source: action.input,
                result: state.result,
                clvm_rs: state.clvm_rs
            }, []];
        } else if (action.run) {
            console.log('run', state);
            let run_clvm = (state.clvm_rs as any).run_clvm;
            console.log('run_clvm', run_clvm);
            let unh = unhexlify(state.source);
            let unh_array = [];
            for (var i = 0; i < unh.length; i++) {
                unh_array[i] = unh.charCodeAt(i);
            }
            let result = run_clvm(new Uint8Array(unh_array), new Uint8Array([128]));
            let result_str = '';
            for (var i = 0; i < result.length; i++) {
                result_str += String.fromCharCode(result[i]);
            }
            return [{
                source: action.input,
                result: hexlify(result_str),
                clvm_rs: state.clvm_rs
            }, []];
        } else {
            console.log('other');
            return [state, []];
        }
    },
    view: state => {
        return <div>
            <h1>Code</h1>
            <input onChange={sender(state.dispatch, (evt) => [{input:evt.target.value}])} value={state.source}></input>
            <h1>Result</h1>
            <pre>{state.result}</pre>
            <button onClick={sender(state.dispatch, () => [{run:true}])}>Run</button>
        </div>;
    }
});

import * as env from './env.js';

import('clvm_rs').then((clvm_rs) => {
    // And use the component!
    env.clvm_rs = clvm_rs;
    render(<App clvm_rs={clvm_rs} />, document.getElementById('app'));
});
