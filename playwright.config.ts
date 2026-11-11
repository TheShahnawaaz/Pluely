import { defineConfig } from '@playwright/test';
export default defineConfig({testDir:'./tests/browser',timeout:30000,use:{baseURL:'http://localhost:1420',viewport:{width:1440,height:980},trace:'retain-on-failure'},webServer:{command:'npm run dev',port:1420,reuseExistingServer:!process.env.CI},reporter:'list'});
