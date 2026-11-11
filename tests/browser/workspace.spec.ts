import {test,expect} from '@playwright/test';
test('workspace, modes, saved transcript and provider settings',async({page})=>{
 const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto('/');await page.getByRole('button',{name:'Explore first'}).click();
 await expect(page.getByRole('heading',{name:'Be present.'})).toBeVisible();
 await page.getByRole('button',{name:'Modes',exact:true}).click();await page.getByRole('button',{name:'Create mode'}).click();
 await page.getByRole('textbox',{name:'Name',exact:true}).fill('Design review');await page.getByRole('textbox',{name:'Live assistance instructions'}).fill('Identify assumptions and useful questions.');await page.getByRole('button',{name:'Save mode',exact:true}).click();
 await expect(page.getByRole('heading',{name:'Design review'})).toBeVisible();
 await page.getByRole('button',{name:'Home',exact:true}).click();await page.getByRole('button',{name:'Explore with clearly labeled example data'}).click();await page.getByRole('button',{name:'Example · Product planning'}).click();
 await page.getByRole('button',{name:'Transcript',exact:true}).click();await expect(page.getByText('Let’s keep the first release local-first, with no account required.')).toBeVisible();
 await page.getByRole('button',{name:'Settings',exact:false}).first().click();await page.getByRole('button',{name:'Transcription',exact:true}).click();await page.getByRole('button',{name:'Local model',exact:true}).click();await expect(page.getByText('Whisper Base',{exact:true})).toBeVisible();expect(errors).toEqual([]);
});
test('browser preview cannot pretend to record',async({page})=>{await page.goto('/');await page.getByRole('button',{name:'Explore first'}).click();await page.getByRole('button',{name:'Start Pluely'}).click();await expect(page.getByRole('alert')).toContainText('requires the native');});
test('setup offers local download/import and either cloud provider',async({page})=>{
 await page.goto('/');await page.getByRole('button',{name:'Continue',exact:true}).click();
 await expect(page.getByText('Device-key encryption · recommended')).toBeVisible();
 await page.getByRole('button',{name:'Continue',exact:true}).click();
 const transcription=page.getByRole('combobox',{name:'Transcription',exact:true});
 await expect(transcription).toHaveValue('local');
 await expect(page.getByText('Whisper Base',{exact:true})).toBeVisible();
 await expect(page.getByRole('button',{name:'Import model file'})).toBeVisible();
 await transcription.selectOption('elevenlabs');
 await expect(page.getByText(/Add your ElevenLabs key/)).toBeVisible();
 await transcription.selectOption('groq');
 await expect(page.getByText(/Add your Groq key/)).toBeVisible();
 await page.getByRole('button',{name:'Open my workspace'}).click();
 await expect(page.getByRole('heading',{name:'Make it yours.'})).toBeVisible();
});
test('overlay expands and collapses without rendering the dashboard',async({page})=>{
 await page.goto('/?window=overlay');
 await expect(page.getByRole('heading',{name:'Right here when you need a hand.'})).toBeVisible();
 await expect(page.getByRole('navigation',{name:'Main navigation'})).toHaveCount(0);
 await page.getByRole('button',{name:'Collapse assistant'}).click();
 await expect(page.getByRole('heading',{name:'Right here when you need a hand.'})).toHaveCount(0);
 await page.getByRole('button',{name:'Expand assistant'}).click();
 await expect(page.getByRole('heading',{name:'Right here when you need a hand.'})).toBeVisible();
});
